use anyhow::Result;
use crossterm::{
    event::{
        self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, MouseButton, MouseEventKind,
    },
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, style::Color, Terminal};
use std::collections::BTreeSet;
use std::io::stdout;
use std::path::Path;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

use super::{CleanContext, CleanFeed, ConfirmChoice, InputMode, ScanFeed, Tui, TuiState};
use crate::cleaner::{CleanEvent, Cleaner, CleanerAction};
use crate::history;
use crate::scanner::ArtifactScanner;
use crate::trash;

/// What a TUI session did, so the shell can say it once the screen is restored.
#[derive(Debug, Default, Clone, Copy)]
pub struct SessionReport {
    pub freed: u64,
    pub cleaned: usize,
    /// True when the user pressed Esc to return to the menu instead of quitting.
    pub back_to_menu: bool,
}

pub struct InteractiveMode;

/// A clean running on a worker thread, feeding the cleaning screen.
struct CleanJob {
    feed: Arc<Mutex<CleanFeed>>,
    handle: JoinHandle<()>,
}

/// Restores the terminal on every exit path, including errors (`?`).
struct TerminalGuard;

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        // Mouse capture must be turned off too, or the shell keeps receiving
        // mouse reports after klean exits.
        let _ = execute!(stdout(), DisableMouseCapture, LeaveAlternateScreen);
    }
}

impl InteractiveMode {
    /// Scans on a worker thread behind the loading screen, then runs the list on
    /// the alternate screen. Cleaning happens here too, so the result stays on
    /// screen instead of flashing past after the UI closes.
    pub fn run_scan(scanner: ArtifactScanner, ctx: CleanContext) -> Result<SessionReport> {
        // The scan runs off-thread so the loading screen can breathe: it names
        // the directory being read and counts folders while it works.
        let feed = Arc::new(Mutex::new(ScanFeed::new()));
        let scan_cancel = {
            let f = feed.lock().unwrap_or_else(|e| e.into_inner());
            Arc::clone(&f.cancel)
        };
        let worker = Arc::clone(&feed);
        let cancel_for_scanner = Arc::clone(&scan_cancel);
        let handle = std::thread::spawn(move || {
            let progress = Arc::clone(&worker);
            let scanner = scanner
                .with_cancel(cancel_for_scanner)
                .with_progress(Box::new(move |path: &Path| {
                if let Ok(mut feed) = progress.lock() {
                    feed.current = Some(path.to_path_buf());
                    feed.dirs += 1;
                }
            }));

            let outcome = scanner.scan();
            let mut feed = worker.lock().unwrap_or_else(|error| error.into_inner());
            feed.done = true;
            feed.outcome = Some(match outcome {
                Ok(outcome) => Ok(outcome.artifacts),
                Err(error) => Err(error.to_string()),
            });
        });

        // Setup terminal. Mouse capture is what makes the wheel scroll the list
        // instead of the terminal scrollback.
        enable_raw_mode()?;
        let mut stdout = stdout();
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
        let guard = TerminalGuard;

        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;

        let mut state = TuiState::new(Vec::new(), ctx.root.clone());
        let mut scanning = true;
        let mut scan_cancelled = false;
        let mut cleaning: Option<CleanJob> = None;
        let mut back_to_menu = false;

        loop {
            // Fold the worker's result in as soon as it lands.
            if scanning {
                let mut feed = feed.lock().unwrap_or_else(|error| error.into_inner());
                if feed.done {
                    if scan_cancelled {
                        // Aborted by the user: go back to menu (or quit).
                        drop(feed);
                        break;
                    }
                    match feed.outcome.take() {
                        Some(Ok(artifacts)) => state = TuiState::new(artifacts, ctx.root.clone()),
                        Some(Err(error)) => state.notice = Some(format!("✗ {error}")),
                        None => {}
                    }
                    scanning = false;
                }
            }

            if cleaning.as_ref().is_some_and(CleanJob::is_done) {
                if let Some(job) = cleaning.take() {
                    finish_clean(&mut state, job);
                }
            }

            terminal.draw(|f| {
                if scanning {
                    let feed = feed.lock().unwrap_or_else(|error| error.into_inner());
                    Tui::render_scanning(f, &feed, &ctx.root);
                    return;
                }
                // The list stays behind every dialog, so the context never
                // disappears.
                Tui::render_list(f, &mut state);
                match state.input_mode {
                    InputMode::Confirmation => {
                        Tui::render_confirmation(f, &state, ctx.backup_dir.is_some())
                    }
                    InputMode::Cleaning => {
                        if let Some(job) = &cleaning {
                            let feed = job.feed.lock().unwrap_or_else(|error| error.into_inner());
                            Tui::render_cleaning(f, &feed, &ctx.root);
                        }
                    }
                    InputMode::Summary => Tui::render_summary(f, &state),
                    _ => {}
                }
            })?;

            // Redraw fast while something moves (spinner, gauge, flash, toast).
            let tick = if scanning || state.animating() {
                50
            } else {
                250
            };
            if event::poll(Duration::from_millis(tick))? {
                match event::read()? {
                    Event::Key(key) => {
                        if key.kind == event::KeyEventKind::Release {
                            continue;
                        }
                        // Cancel or quit during the loading/scanning screen.
                        if scanning {
                            match key.code {
                                KeyCode::Esc => {
                                    scan_cancel.store(true, std::sync::atomic::Ordering::Relaxed);
                                    scan_cancelled = true;
                                    back_to_menu = true;
                                }
                                KeyCode::Char('q') | KeyCode::Char('Q') => {
                                    scan_cancel.store(true, std::sync::atomic::Ordering::Relaxed);
                                    scan_cancelled = true;
                                }
                                _ => {}
                            }
                            continue;
                        }
                        match state.input_mode {
                            InputMode::Normal => match key.code {
                                KeyCode::Char('q') | KeyCode::Char('Q') => {
                                    state.input_mode = InputMode::Exiting;
                                    break;
                                }
                                KeyCode::Esc => {
                                    back_to_menu = true;
                                    state.input_mode = InputMode::Exiting;
                                    break;
                                }
                                KeyCode::Up | KeyCode::Char('k') => state.move_up(),
                                KeyCode::Down | KeyCode::Char('j') => state.move_down(),
                                KeyCode::PageUp => state.move_page(false),
                                KeyCode::PageDown => state.move_page(true),
                                KeyCode::Home => state.move_to_first(),
                                KeyCode::End => state.move_to_last(),
                                // Mark and step to the next row: a run of
                                // Space presses marks a block.
                                KeyCode::Char(' ') | KeyCode::Char('x') => {
                                    state.toggle_selected();
                                    state.move_down();
                                }
                                KeyCode::Char('a') | KeyCode::Char('A') => state.select_all(),
                                KeyCode::Char('d') | KeyCode::Char('D') => state.deselect_all(),
                                KeyCode::Char('i') | KeyCode::Char('I') => state.invert_selection(),
                                KeyCode::Enter => {
                                    if state.selected_count() == 0 {
                                        state.show_toast(
                                            "nothing selected — Space marks the row, A marks all",
                                            Color::Yellow,
                                        );
                                    } else {
                                        state.confirm_choice = ConfirmChoice::Clean;
                                        state.input_mode = InputMode::Confirmation;
                                    }
                                }
                                _ => {}
                            },
                            InputMode::Confirmation => {
                                let chosen = match key.code {
                                    KeyCode::Left | KeyCode::BackTab | KeyCode::Char('h') => {
                                        state.confirm_choice = state.confirm_choice.previous();
                                        None
                                    }
                                    KeyCode::Right | KeyCode::Tab | KeyCode::Char('l') => {
                                        state.confirm_choice = state.confirm_choice.next();
                                        None
                                    }
                                    KeyCode::Enter => Some(state.confirm_choice),
                                    KeyCode::Char('y') | KeyCode::Char('Y') => {
                                        Some(ConfirmChoice::Clean)
                                    }
                                    // Trash instead of delete: `U` (or `klean
                                    // undo`) brings it all back.
                                    KeyCode::Char('t') | KeyCode::Char('T') => {
                                        Some(ConfirmChoice::Trash)
                                    }
                                    KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                                        Some(ConfirmChoice::Cancel)
                                    }
                                    _ => None,
                                };
                                match chosen {
                                    Some(ConfirmChoice::Cancel) => {
                                        state.input_mode = InputMode::Normal;
                                        state.show_toast("cleanup cancelled", Color::Gray);
                                    }
                                    Some(ConfirmChoice::Clean) => {
                                        let action = if ctx.backup_dir.is_some() {
                                            CleanerAction::Backup
                                        } else {
                                            CleanerAction::Delete
                                        };
                                        cleaning = start_clean(&mut state, &ctx, action);
                                    }
                                    Some(ConfirmChoice::Trash) => {
                                        cleaning =
                                            start_clean(&mut state, &ctx, CleanerAction::Trash);
                                    }
                                    None => {}
                                }
                            }
                            // Nothing is ever left half-deleted: Esc stops the
                            // run before the next artifact.
                            InputMode::Cleaning => {
                                if matches!(key.code, KeyCode::Esc | KeyCode::Char('q')) {
                                    if let Some(job) = &cleaning {
                                        let feed = job
                                            .feed
                                            .lock()
                                            .unwrap_or_else(|error| error.into_inner());
                                        feed.cancel.store(true, Ordering::Relaxed);
                                    }
                                }
                            }
                            // The result stays until it is dismissed; Q still exits.
                            InputMode::Summary => match key.code {
                                KeyCode::Char('q') | KeyCode::Esc => break,
                                KeyCode::Char('u') | KeyCode::Char('U') => {
                                    match trash::undo_last() {
                                        Ok(Some(report)) if !report.restored.is_empty() => {
                                            state.apply_undo(&report.restored)
                                        }
                                        Ok(_) => {
                                            state.last_result = None;
                                            state.notice =
                                                Some("nothing in trash to undo".to_string());
                                            state.input_mode = InputMode::Normal;
                                        }
                                        Err(error) => {
                                            state.last_result = None;
                                            state.notice = Some(format!("✗ {error}"));
                                            state.input_mode = InputMode::Normal;
                                        }
                                    }
                                }
                                _ => {
                                    state.last_result = None;
                                    state.input_mode = InputMode::Normal;
                                }
                            },
                            InputMode::Exiting => break,
                            _ => {}
                        }
                    }
                    // Wheel scrolling and clicks, available because of
                    // EnableMouseCapture.
                    Event::Mouse(mouse) if state.input_mode == InputMode::Normal => {
                        match mouse.kind {
                            MouseEventKind::ScrollDown => state.move_down(),
                            MouseEventKind::ScrollUp => state.move_up(),
                            MouseEventKind::ScrollRight => state.move_page(true),
                            MouseEventKind::ScrollLeft => state.move_page(false),
                            MouseEventKind::Down(MouseButton::Left) => {
                                state.click(mouse.column, mouse.row)
                            }
                            _ => {}
                        }
                    }
                    _ => {}
                }
            }
        }

        let report = SessionReport {
            freed: state.session_freed,
            cleaned: state.session_cleaned,
            back_to_menu,
        };
        let _ = handle.join();
        // Restore terminal before the caller prints anything.
        drop(guard);

        Ok(report)
    }
}

impl CleanJob {
    fn is_done(&self) -> bool {
        self.feed
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .done
    }
}

/// Checks the selection, then cleans it on a worker thread that feeds the
/// cleaning screen and records the run in the history file.
fn start_clean(
    state: &mut TuiState,
    ctx: &CleanContext,
    action: CleanerAction,
) -> Option<CleanJob> {
    let to_clean = state.get_selected_artifacts();

    // Refusals (too big, nested, system paths) are cheap: answer before any
    // thread starts, and leave the selection as it was.
    let check = Cleaner::new(action, ctx.backup_dir.clone(), ctx.allow_system_paths)
        .with_root(ctx.root.clone())
        .with_silent(true);
    if let Err(error) = check.verify_safety(&to_clean) {
        state.notice = Some(format!("✗ {error}"));
        state.input_mode = InputMode::Normal;
        return None;
    }

    let feed = Arc::new(Mutex::new(CleanFeed::new(action, to_clean.clone())));
    let cancel = feed
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .cancel
        .clone();
    let worker = Arc::clone(&feed);
    let (backup_dir, root, allow_system_paths) = (
        ctx.backup_dir.clone(),
        ctx.root.clone(),
        ctx.allow_system_paths,
    );

    let handle = std::thread::spawn(move || {
        let progress = Arc::clone(&worker);
        let cleaner = Cleaner::new(action, backup_dir, allow_system_paths)
            .with_root(root.clone())
            // The TUI owns the screen: a progress bar here would be painted on
            // top of the table.
            .with_silent(true)
            .with_cancel(cancel)
            .with_progress(Box::new(move |event: CleanEvent<'_>| {
                if let Ok(mut feed) = progress.lock() {
                    feed.record(event);
                }
            }));

        let action_label = match action {
            CleanerAction::Trash => "trash",
            CleanerAction::Backup => "backup",
            CleanerAction::Delete => "delete",
        };
        let projects = to_clean
            .iter()
            .map(|artifact| artifact.project.clone())
            .collect::<BTreeSet<_>>()
            .len();

        let outcome = cleaner.clean(to_clean, false).and_then(|result| {
            history::record(
                &root,
                action_label,
                result.total_size_freed,
                result.deleted + result.backed_up,
                projects,
            )?;
            Ok(result)
        });

        let mut feed = worker.lock().unwrap_or_else(|error| error.into_inner());
        feed.finish(outcome.map_err(|error| error.to_string()));
    });

    state.notice = None;
    state.input_mode = InputMode::Cleaning;
    Some(CleanJob { feed, handle })
}

/// Folds a finished worker back into the screen: the summary on success, the
/// error (and whatever did get removed) otherwise.
fn finish_clean(state: &mut TuiState, job: CleanJob) {
    let _ = job.handle.join();
    let mut feed = job.feed.lock().unwrap_or_else(|error| error.into_inner());
    let run = feed.run();
    match feed.outcome.take() {
        Some(Ok(result)) => state.apply_clean_result(&result, run),
        Some(Err(error)) => {
            state.prune_missing();
            state.notice = Some(format!("✗ {error}"));
            state.input_mode = InputMode::Normal;
        }
        None => state.input_mode = InputMode::Normal,
    }
}
