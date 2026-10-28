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

const ITEMS: &[(MenuChoice, &str, &str)] = &[
    (
        MenuChoice::Clean,
        "Limpar artefatos",
        "node_modules, target, .cache, dist, …",
    ),
    (
        MenuChoice::Globals,
        "Pacotes globais",
        "npm, cargo, pip, brew, binários",
    ),
    (MenuChoice::Stats, "Estatísticas", "histórico de limpeza"),
    (MenuChoice::Quit, "Sair", ""),
];

struct State {
    cursor: usize,
    /// Background version check result (None = still checking).
    version_info: Arc<Mutex<Option<VersionInfo>>>,
    version_done: Arc<AtomicBool>,
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
        terminal.draw(|f| render(f, &state, version))?;

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
                    if state.cursor + 1 < ITEMS.len() {
                        state.cursor += 1;
                    }
                }
                KeyCode::Char('1') => return Ok(MenuChoice::Clean),
                KeyCode::Char('2') => return Ok(MenuChoice::Globals),
                KeyCode::Char('3') => return Ok(MenuChoice::Stats),
                KeyCode::Char('4') | KeyCode::Char('q') | KeyCode::Esc => {
                    return Ok(MenuChoice::Quit)
                }
                KeyCode::Enter => {
                    return Ok(ITEMS[state.cursor].0.clone());
                }
                _ => {}
            }
        }
    }
}

fn render(f: &mut Frame, state: &State, version: &str) {
    let size = f.area();

    // Read version info without blocking (best-effort).
    let vi: Option<VersionInfo> = if state.version_done.load(Ordering::SeqCst) {
        state.version_info.lock().ok().and_then(|g| g.clone())
    } else {
        None
    };

    // Extra line for update banner when available.
    let has_update = vi.as_ref().map(|v| v.update_available).unwrap_or(false);
    let extra = if has_update { 2u16 } else { 0 };

    // Center vertically: logo(6)+gap(1)+version(1)+gap(1)+items(4)+gap(1)+footer(1) + optional banner
    let content_height = 15u16 + extra;
    let v_pad = size.height.saturating_sub(content_height) / 2;

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(v_pad),
            Constraint::Length(8), // logo + gap + version line
            Constraint::Length(if has_update { 2 } else { 0 }),
            Constraint::Min(1), // menu items + footer
        ])
        .split(size);

    // ── Logo ──────────────────────────────────────────────────────────────────
    let version_line = if vi.is_none() && !state.version_done.load(Ordering::SeqCst) {
        Line::from(Span::styled(
            format!("v{}  •  verificando atualizações…", version),
            Style::default().fg(Color::DarkGray),
        ))
    } else {
        Line::from(Span::styled(
            format!(
                "v{}  •  gerenciador de ambientes de desenvolvimento",
                version
            ),
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

    let logo_widget = Paragraph::new(logo_lines).alignment(Alignment::Center);
    f.render_widget(logo_widget, chunks[1]);

    // ── Update banner ─────────────────────────────────────────────────────────
    if has_update {
        if let Some(ref info) = vi {
            let banner = Paragraph::new(vec![
                Line::from(vec![
                    Span::styled(
                        "  ⬆  Nova versão disponível: ",
                        Style::default().fg(Color::Yellow),
                    ),
                    Span::styled(
                        format!("v{}", info.latest),
                        Style::default()
                            .fg(Color::Yellow)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled("  —  execute ", Style::default().fg(Color::DarkGray)),
                    Span::styled(
                        "klean update",
                        Style::default()
                            .fg(Color::Cyan)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(" para atualizar", Style::default().fg(Color::DarkGray)),
                ]),
                Line::from(""),
            ])
            .alignment(Alignment::Center);
            f.render_widget(banner, chunks[2]);
        }
    }

    // ── Menu items ────────────────────────────────────────────────────────────
    let item_lines: Vec<Line> = ITEMS
        .iter()
        .enumerate()
        .map(|(i, (_, label, hint))| {
            let is_sel = i == state.cursor;
            let num = format!("{}", i + 1);
            if is_sel {
                Line::from(vec![
                    Span::styled(
                        format!("  ▶  {}. ", num),
                        Style::default()
                            .fg(Color::Cyan)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        *label,
                        Style::default()
                            .fg(Color::White)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        if hint.is_empty() {
                            String::new()
                        } else {
                            format!("  — {}", hint)
                        },
                        Style::default().fg(Color::DarkGray),
                    ),
                ])
            } else {
                Line::from(vec![
                    Span::styled(
                        format!("     {}. ", num),
                        Style::default().fg(Color::DarkGray),
                    ),
                    Span::styled(*label, Style::default().fg(Color::Gray)),
                    Span::styled(
                        if hint.is_empty() {
                            String::new()
                        } else {
                            format!("  — {}", hint)
                        },
                        Style::default().fg(Color::DarkGray),
                    ),
                ])
            }
        })
        .chain(std::iter::once(Line::from("")))
        .chain(std::iter::once(Line::from(Span::styled(
            "  ↑↓/jk navegar   Enter selecionar   1-4 acesso rápido",
            Style::default().fg(Color::DarkGray),
        ))))
        .collect();

    let menu_widget = Paragraph::new(item_lines)
        .block(Block::default().borders(Borders::NONE))
        .alignment(Alignment::Left);
    f.render_widget(menu_widget, chunks[3]);
}
