//! Inline-viewport event loop: owns the worker task that runs `AgentSession`
//! turns, the synchronous terminal loop that reads input and flushes
//! finalized messages into the terminal's native scrollback, and terminal
//! restoration on every exit path.

mod app;
mod view;

use std::io;
use std::time::Duration;

use ratatui::crossterm::event::{
    self, DisableBracketedPaste, EnableBracketedPaste, Event, KeyEventKind,
};
use ratatui::crossterm::execute;
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

    // Unbounded, not merely convenient: the synchronous terminal loop below cannot
    // `.await` a bounded `Sender::send`, and `UnboundedSender::send` /
    // `UnboundedReceiver::try_recv` are both non-async. `App`'s `Status::Idle` gate
    // keeps at most one prompt in flight, so the queue depth stays bounded in
    // practice even though the channel type isn't. See
    // docs/decisions/0003-sync-terminal-loop-explicit-runtime.md.
    let (command_tx, mut command_rx) = mpsc::unbounded_channel::<String>();
    let (result_tx, mut result_rx) = mpsc::unbounded_channel::<WorkerEvent>();

    let worker = runtime.spawn(async move {
        while let Some(prompt) = command_rx.recv().await {
            // A clone per prompt, so the observer this turn reports through is
            // dropped with the turn. A closed channel means the terminal loop
            // is already gone; the send below notices and ends the worker.
            let tool_tx = result_tx.clone();
            let observed = session.prompt(&prompt, move |call| {
                let _ = tool_tx.send(WorkerEvent::ToolCall(call));
            });
            let event = match observed.await {
                Ok(reply) => WorkerEvent::Reply(reply),
                Err(err) => WorkerEvent::Failed(err),
            };
            if result_tx.send(event).is_err() {
                break;
            }
        }
    });

    // Enabling bracketed paste is folded into `loop_result` rather than run
    // with `?` above: a failure here must still reach `restore` below, or the
    // terminal is left in raw mode (I7 in docs/architecture.md).
    let loop_result = enable_bracketed_paste()
        .and_then(|()| event_loop(&mut terminal, &command_tx, &mut result_rx));
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

fn enable_bracketed_paste() -> anyhow::Result<()> {
    execute!(io::stdout(), EnableBracketedPaste)?;
    Ok(())
}

/// Undoes every terminal mode `run` turned on, and clears the inline viewport.
///
/// `try_restore` alone leaves the live region's rows printed as leftover
/// garbage: it disables raw mode but does not erase the viewport. Bracketed
/// paste has to be turned off too, or the user's shell keeps receiving paste
/// markers after `fey` exits.
///
/// Every step runs even when an earlier one failed. Skipping the rest after the
/// first error is what would leave a terminal unusable, which is the one thing
/// I7 (docs/architecture.md) rules out.
fn restore(terminal: &mut DefaultTerminal) -> anyhow::Result<()> {
    let mut failures = [
        execute!(io::stdout(), DisableBracketedPaste).map_err(anyhow::Error::from),
        terminal.clear().map_err(anyhow::Error::from),
        ratatui::try_restore().map_err(anyhow::Error::from),
    ]
    .into_iter()
    .filter_map(Result::err);

    let Some(first) = failures.next() else {
        return Ok(());
    };
    Err(failures.fold(first, |reported, also| {
        reported.context(format!("another restoration step also failed: {also}"))
    }))
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
                let message = app.receive(event);
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
                Event::Paste(text) => {
                    if app.paste(&text) == Action::Edit {
                        dirty = true;
                    }
                }
                Event::Resize(_, _) => dirty = true,
                _ => {}
            }
        }
    }
}

/// Writes a finalized message into the terminal's native scrollback.
///
/// `insert_before` takes the height as a `u16`, so a message taller than that
/// is written in several passes. Casting the length instead would wrap and drop
/// most of the message without any sign that it happened.
fn flush_message(terminal: &mut DefaultTerminal, message: &Message) -> anyhow::Result<()> {
    let width = terminal.size()?.width;
    let mut lines = wrap_message(message, width);

    while !lines.is_empty() {
        let taken = lines.len().min(u16::MAX as usize);
        let rest = lines.split_off(taken);
        terminal.insert_before(taken as u16, move |buf| render_message(lines, buf))?;
        lines = rest;
    }

    Ok(())
}
