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
use std::time::Duration;

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
}

struct TermGuard;

impl Drop for TermGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(stdout(), DisableMouseCapture, LeaveAlternateScreen);
    }
}

pub fn run_menu(version: &str) -> Result<MenuChoice> {
    enable_raw_mode()?;
    execute!(stdout(), EnterAlternateScreen, EnableMouseCapture)?;
    let _guard = TermGuard;

    let backend = CrosstermBackend::new(stdout());
    let mut terminal = Terminal::new(backend)?;
    let mut state = State { cursor: 0 };

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

    // Center vertically: logo(6) + gap(1) + version(1) + gap(1) + items(4) + gap(1) + footer(1) = 15
    let content_height = 15u16;
    let v_pad = size.height.saturating_sub(content_height) / 2;

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(v_pad),
            Constraint::Length(8), // logo + gap + version
            Constraint::Length(content_height.saturating_sub(8).max(1)),
            Constraint::Min(0),
        ])
        .split(size);

    // Logo
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
        .chain(std::iter::once(Line::from(Span::styled(
            format!(
                "v{}  •  gerenciador de ambientes de desenvolvimento",
                version
            ),
            Style::default().fg(Color::DarkGray),
        ))))
        .collect();

    let logo_widget = Paragraph::new(logo_lines).alignment(Alignment::Center);
    f.render_widget(logo_widget, chunks[1]);

    // Menu items
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
    f.render_widget(menu_widget, chunks[2]);
}
