//! Inline-viewport event loop: owns the worker task that runs `AgentSession`
//! turns, the synchronous terminal loop that reads input and flushes
//! finalized messages into the terminal's native scrollback, and terminal
//! restoration on every exit path.

mod app;
mod view;

use std::time::Duration;

use ratatui::crossterm::event::{self, Event, KeyEventKind};
use ratatui::{DefaultTerminal, TerminalOptions, Viewport};
use tokio::sync::mpsc;
use tokio::sync::mpsc::error::TryRecvError;

use fey::AgentSession;

use app::{Action, App, Message, WorkerEvent};
use view::{LIVE_REGION_HEIGHT, render_live_region, render_message, wrap_message};

const POLL_INTERVAL: Duration = Duration::from_millis(50);

pub(crate) fn run(runtime: &tokio::runtime::Runtime, session: AgentSession) -> anyhow::Result<()> {
    let mut terminal = ratatui::try_init_with_options(TerminalOptions {
        viewport: Viewport::Inline(LIVE_REGION_HEIGHT),
    })?;

    let (command_tx, mut command_rx) = mpsc::unbounded_channel::<String>();
    let (result_tx, mut result_rx) = mpsc::unbounded_channel::<WorkerEvent>();

    let worker = runtime.spawn(async move {
        while let Some(prompt) = command_rx.recv().await {
            let event = match session.prompt(&prompt).await {
                Ok(reply) => WorkerEvent::Reply(reply),
                Err(err) => WorkerEvent::Failed(err),
            };
            if result_tx.send(event).is_err() {
                break;
            }
        }
    });

    let loop_result = event_loop(&mut terminal, &command_tx, &mut result_rx);
    drop(command_tx);
    worker.abort();

    let restore_result = restore(&mut terminal);

    match (loop_result, restore_result) {
        (Ok(()), Ok(())) => Ok(()),
        (Err(loop_err), Ok(())) => Err(loop_err),
        (Ok(()), Err(restore_err)) => Err(restore_err),
        (Err(loop_err), Err(restore_err)) => {
            Err(loop_err.context(format!("terminal restoration also failed: {restore_err}")))
        }
    }
}

/// Clears the inline viewport and restores raw mode. `try_restore` alone
/// leaves the live region's rows printed as leftover garbage: it disables
/// raw mode but does not erase the viewport.
fn restore(terminal: &mut DefaultTerminal) -> anyhow::Result<()> {
    let clear_result = terminal.clear().map_err(anyhow::Error::from);
    let raw_mode_result = ratatui::try_restore().map_err(anyhow::Error::from);

    match (clear_result, raw_mode_result) {
        (Ok(()), Ok(())) => Ok(()),
        (Err(err), Ok(())) => Err(err),
        (Ok(()), Err(err)) => Err(err),
        (Err(clear_err), Err(raw_mode_err)) => {
            Err(clear_err.context(format!("raw mode restoration also failed: {raw_mode_err}")))
        }
    }
}

fn event_loop(
    terminal: &mut DefaultTerminal,
    command_tx: &mpsc::UnboundedSender<String>,
    result_rx: &mut mpsc::UnboundedReceiver<WorkerEvent>,
) -> anyhow::Result<()> {
    let mut app = App::new();
    let mut dirty = true;

    loop {
        if dirty {
            terminal.draw(|frame| render_live_region(frame, frame.area(), &app))?;
            dirty = false;
        }

        match result_rx.try_recv() {
            Ok(event) => {
                let message = app.complete(event);
                flush_message(terminal, &message)?;
                dirty = true;
            }
            Err(TryRecvError::Empty) => {}
            Err(TryRecvError::Disconnected) => {
                if let Some(stopped) = app.stop() {
                    flush_message(terminal, &stopped)?;
                    dirty = true;
                }
            }
        }

        if event::poll(POLL_INTERVAL)? {
            match event::read()? {
                Event::Key(key) if key.kind == KeyEventKind::Press => match app.handle_key(key) {
                    Action::Edit => dirty = true,
                    Action::Submit(message) => {
                        flush_message(terminal, &message)?;
                        if command_tx.send(message.text).is_err()
                            && let Some(stopped) = app.stop()
                        {
                            flush_message(terminal, &stopped)?;
                        }
                        dirty = true;
                    }
                    Action::Quit => return Ok(()),
                    Action::None => {}
                },
                Event::Resize(_, _) => dirty = true,
                _ => {}
            }
        }
    }
}

fn flush_message(terminal: &mut DefaultTerminal, message: &Message) -> anyhow::Result<()> {
    let width = terminal.size()?.width;
    let lines = wrap_message(message, width);
    let height = lines.len() as u16;
    terminal.insert_before(height, move |buf| render_message(lines, buf))?;
    Ok(())
}
