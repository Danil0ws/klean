//! Main menu TUI shown at startup in interactive mode.

use anyhow::Result;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame, Terminal,
};
use std::io::stdout;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::update::VersionInfo;

#[derive(Debug, Clone, PartialEq)]
pub enum MenuChoice {
    Clean,
    Globals,
    Stats,
    Update,
    Quit,
}

const LOGO: [&str; 6] = [
    r" ██╗  ██╗██╗     ███████╗ █████╗ ███╗   ██╗",
    r" ██║ ██╔╝██║     ██╔════╝██╔══██╗████╗  ██║",
    r" █████╔╝ ██║     █████╗  ███████║██╔██╗ ██║",
    r" ██╔═██╗ ██║     ██╔══╝  ██╔══██║██║╚██╗██║",
    r" ██║  ██╗███████╗███████╗██║  ██║██║ ╚████║",
    r" ╚═╝  ╚═╝╚══════╝╚══════╝╚═╝  ╚═╝╚═╝  ╚═══╝",
];

/// Static items always shown (index = display number - 1).
const BASE_ITEMS: &[(MenuChoice, &str, &str)] = &[
    (
        MenuChoice::Clean,
        "Clean artifacts",
        "node_modules, target, .cache, dist, …",
    ),
    (
        MenuChoice::Globals,
        "Global packages",
        "npm, cargo, pip, brew, binaries",
    ),
    (MenuChoice::Stats, "Statistics", "cleanup history"),
    (MenuChoice::Quit, "Quit", ""),
];

struct State {
    cursor: usize,
    version_info: Arc<Mutex<Option<VersionInfo>>>,
    version_done: Arc<AtomicBool>,
}

impl State {
    /// Returns the version info only after the background check completes.
    fn version_info(&self) -> Option<VersionInfo> {
        if self.version_done.load(Ordering::SeqCst) {
            self.version_info.lock().ok().and_then(|g| g.clone())
        } else {
            None
        }
    }

    fn menu_len(&self, has_update: bool) -> usize {
        BASE_ITEMS.len() + if has_update { 1 } else { 0 }
    }
}

struct TermGuard;

impl Drop for TermGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(stdout(), DisableMouseCapture, LeaveAlternateScreen);
    }
}

pub fn run_menu(version: &str) -> Result<MenuChoice> {
    // Kick off background version check immediately.
    let version_info: Arc<Mutex<Option<VersionInfo>>> = Arc::new(Mutex::new(None));
    let version_done = Arc::new(AtomicBool::new(false));
    {
        let info_slot = Arc::clone(&version_info);
        let done = Arc::clone(&version_done);
        std::thread::spawn(move || {
            if let Ok(info) = VersionInfo::fetch() {
                *info_slot.lock().unwrap() = Some(info);
            }
            done.store(true, Ordering::SeqCst);
        });
    }

    enable_raw_mode()?;
    execute!(stdout(), EnterAlternateScreen, EnableMouseCapture)?;
    let _guard = TermGuard;

    let backend = CrosstermBackend::new(stdout());
    let mut terminal = Terminal::new(backend)?;
    let mut state = State {
        cursor: 0,
        version_info,
        version_done,
    };

    loop {
        let vi = state.version_info();
        let has_update = vi.as_ref().map(|v| v.update_available).unwrap_or(false);
        let menu_len = state.menu_len(has_update);

        terminal.draw(|f| render(f, &state, version, vi.as_ref()))?;

        if !event::poll(Duration::from_millis(100))? {
            continue;
        }

        if let Event::Key(key) = event::read()? {
            match key.code {
                KeyCode::Up | KeyCode::Char('k') => {
                    if state.cursor > 0 {
                        state.cursor -= 1;
                    }
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    if state.cursor + 1 < menu_len {
                        state.cursor += 1;
                    }
                }
                KeyCode::Char('1') => return Ok(MenuChoice::Clean),
                KeyCode::Char('2') => return Ok(MenuChoice::Globals),
                KeyCode::Char('3') => return Ok(MenuChoice::Stats),
                KeyCode::Char('4') if !has_update => return Ok(MenuChoice::Quit),
                KeyCode::Char('4') if has_update => return Ok(MenuChoice::Update),
                KeyCode::Char('5') if has_update => return Ok(MenuChoice::Quit),
                KeyCode::Char('q') | KeyCode::Esc => return Ok(MenuChoice::Quit),
                KeyCode::Enter => {
                    let choice = item_at(state.cursor, has_update);
                    return Ok(choice);
                }
                _ => {}
            }
        }
    }
}

fn item_at(idx: usize, has_update: bool) -> MenuChoice {
    // Items: 0=Clean 1=Globals 2=Stats [3=Update if avail] last=Quit
    if !has_update || idx < 3 {
        BASE_ITEMS[idx].0.clone()
    } else if idx == 3 {
        MenuChoice::Update
    } else {
        MenuChoice::Quit
    }
}

fn render(f: &mut Frame, state: &State, version: &str, vi: Option<&VersionInfo>) {
    let size = f.area();

    let has_update = vi.map(|v| v.update_available).unwrap_or(false);
    let update_item_count = if has_update { 1usize } else { 0 };
    // base items + optional update item = total interactive rows
    let total_items = BASE_ITEMS.len() + update_item_count;
    // layout: logo(6) + gap(1) + version(1) + gap(1) + items + gap(1) + footer(1)
    let content_height = (11 + total_items) as u16;
    let v_pad = size.height.saturating_sub(content_height) / 2;

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(v_pad),
            Constraint::Length(8), // logo + gap + version line
            Constraint::Min(1),    // menu items + footer
        ])
        .split(size);

    // ── Logo ─────────────────────────────────────────────────────────────────
    let version_line = if !state.version_done.load(Ordering::SeqCst) {
        Line::from(Span::styled(
            format!("v{}  •  checking for updates…", version),
            Style::default().fg(Color::DarkGray),
        ))
    } else {
        Line::from(Span::styled(
            format!("v{}  •  development environment manager", version),
            Style::default().fg(Color::DarkGray),
        ))
    };

    let logo_lines: Vec<Line> = LOGO
        .iter()
        .map(|l| {
            Line::from(Span::styled(
                *l,
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ))
        })
        .chain(std::iter::once(Line::from("")))
        .chain(std::iter::once(version_line))
        .collect();

    f.render_widget(
        Paragraph::new(logo_lines).alignment(Alignment::Center),
        chunks[1],
    );

    // ── Menu items ────────────────────────────────────────────────────────────
    // Build the full item list dynamically: base items with "Quit" last, inserting
    // "Update" before "Quit" when an update is available.
    let mut items: Vec<(MenuChoice, String, &str)> = BASE_ITEMS[..3]
        .iter()
        .map(|(c, l, h)| (c.clone(), l.to_string(), *h))
        .collect();

    if has_update {
        let latest = vi.map(|v| v.latest.as_str()).unwrap_or("?");
        items.push((
            MenuChoice::Update,
            format!("Update  →  v{}", latest),
            "new version available",
        ));
    }

    items.push((MenuChoice::Quit, "Quit".to_string(), ""));

    let mut lines: Vec<Line> = items
        .iter()
        .enumerate()
        .map(|(i, (choice, label, hint))| {
            let is_sel = i == state.cursor;
            let num = i + 1;
            let is_update = *choice == MenuChoice::Update;

            let num_span = if is_sel {
                Span::styled(
                    format!("  ▶  {}. ", num),
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                )
            } else {
                Span::styled(
                    format!("     {}. ", num),
                    Style::default().fg(Color::DarkGray),
                )
            };

            let label_style = if is_update {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else if is_sel {
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Gray)
            };

            let hint_span = Span::styled(
                if hint.is_empty() {
                    String::new()
                } else {
                    format!("  — {}", hint)
                },
                Style::default().fg(if is_update {
                    Color::Yellow
                } else {
                    Color::DarkGray
                }),
            );

            Line::from(vec![
                num_span,
                Span::styled(label.as_str(), label_style),
                hint_span,
            ])
        })
        .collect();

    let shortcut_max = if has_update { "1-5" } else { "1-4" };
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        format!(
            "  ↑↓/jk navigate   Enter select   {} quick-access",
            shortcut_max
        ),
        Style::default().fg(Color::DarkGray),
    )));

    f.render_widget(
        Paragraph::new(lines)
            .block(Block::default().borders(Borders::NONE))
            .alignment(Alignment::Left),
        chunks[2],
    );
}
