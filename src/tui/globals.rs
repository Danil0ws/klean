//! Interactive TUI for managing global npm/cargo/pip packages.

use anyhow::Result;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use humansize::format_size;
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Clear, Gauge, Paragraph, Row, Table, TableState},
    Frame, Terminal,
};
use std::collections::BTreeSet;
use std::io::stdout;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::globals::{scan_all, uninstall, GlobalPackage, Manager};

pub struct GlobalsReport {
    pub removed: usize,
    pub freed: u64,
}

#[derive(Debug, Clone, PartialEq)]
enum UiState {
    Scanning,
    Listing,
    Confirming,
    Removing,
    Done,
}

struct State {
    packages: Vec<GlobalPackage>,
    selected: BTreeSet<usize>,
    table: TableState,
    ui: UiState,
    remove_log: Vec<String>,
    remove_idx: usize,
    report: GlobalsReport,
}

impl State {
    fn new() -> Self {
        let mut table = TableState::default();
        table.select(Some(0));
        Self {
            packages: Vec::new(),
            selected: BTreeSet::new(),
            table,
            ui: UiState::Scanning,
            remove_log: Vec::new(),
            remove_idx: 0,
            report: GlobalsReport {
                removed: 0,
                freed: 0,
            },
        }
    }

    fn toggle_current(&mut self) {
        if let Some(i) = self.table.selected() {
            if self.selected.contains(&i) {
                self.selected.remove(&i);
            } else {
                self.selected.insert(i);
            }
        }
    }

    fn select_all(&mut self) {
        if self.selected.len() == self.packages.len() {
            self.selected.clear();
        } else {
            self.selected = (0..self.packages.len()).collect();
        }
    }

    fn move_cursor(&mut self, delta: i64) {
        let len = self.packages.len();
        if len == 0 {
            return;
        }
        let cur = self.table.selected().unwrap_or(0) as i64;
        let next = ((cur + delta).rem_euclid(len as i64)) as usize;
        self.table.select(Some(next));
    }

    fn selected_packages(&self) -> Vec<GlobalPackage> {
        self.selected
            .iter()
            .filter_map(|&i| self.packages.get(i).cloned())
            .collect()
    }
}

struct TermGuard;

impl Drop for TermGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(stdout(), DisableMouseCapture, LeaveAlternateScreen);
    }
}

pub fn run_globals(dry_run: bool) -> Result<GlobalsReport> {
    let scan_result: Arc<Mutex<Option<Vec<GlobalPackage>>>> = Arc::new(Mutex::new(None));
    let scan_done = Arc::new(AtomicBool::new(false));
    {
        let result = Arc::clone(&scan_result);
        let done = Arc::clone(&scan_done);
        std::thread::spawn(move || {
            let packages = scan_all();
            *result.lock().unwrap() = Some(packages);
            done.store(true, Ordering::SeqCst);
        });
    }

    enable_raw_mode()?;
    execute!(stdout(), EnterAlternateScreen, EnableMouseCapture)?;
    let _guard = TermGuard;

    let backend = CrosstermBackend::new(stdout());
    let mut terminal = Terminal::new(backend)?;
    let mut state = State::new();

    loop {
        terminal.draw(|f| render(f, &mut state))?;

        // Absorb scan result when ready
        if state.ui == UiState::Scanning && scan_done.load(Ordering::SeqCst) {
            if let Ok(mut guard) = scan_result.lock() {
                if let Some(pkgs) = guard.take() {
                    state.packages = pkgs;
                    state.packages.sort_by(|a, b| {
                        a.manager
                            .label()
                            .cmp(b.manager.label())
                            .then(a.name.cmp(&b.name))
                    });
                    state.table.select(if state.packages.is_empty() {
                        None
                    } else {
                        Some(0)
                    });
                    state.ui = UiState::Listing;
                }
            }
        }

        // Step through removal without blocking on key events
        if state.ui == UiState::Removing {
            let targets = state.selected_packages();
            if state.remove_idx < targets.len() {
                let pkg = targets[state.remove_idx].clone();
                match uninstall(&pkg, dry_run) {
                    Ok(_) => {
                        state.report.removed += 1;
                        state.report.freed += pkg.size_bytes;
                        state.remove_log.push(format!(
                            "✓ {} {} ({})",
                            pkg.manager.icon(),
                            pkg.name,
                            pkg.version
                        ));
                    }
                    Err(e) => {
                        state.remove_log.push(format!(
                            "✗ {} {} — {}",
                            pkg.manager.icon(),
                            pkg.name,
                            e
                        ));
                    }
                }
                state.remove_idx += 1;
            } else {
                state.ui = UiState::Done;
            }
            continue;
        }

        if !event::poll(Duration::from_millis(100))? {
            continue;
        }

        if let Event::Key(key) = event::read()? {
            match state.ui {
                UiState::Scanning => {
                    if key.code == KeyCode::Char('q') || key.code == KeyCode::Esc {
                        return Ok(GlobalsReport {
                            removed: 0,
                            freed: 0,
                        });
                    }
                }
                UiState::Listing => match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => return Ok(state.report),
                    KeyCode::Down | KeyCode::Char('j') => state.move_cursor(1),
                    KeyCode::Up | KeyCode::Char('k') => state.move_cursor(-1),
                    KeyCode::Char(' ') => state.toggle_current(),
                    KeyCode::Char('a') => state.select_all(),
                    KeyCode::Enter if !state.selected.is_empty() => {
                        state.ui = UiState::Confirming;
                    }
                    _ => {}
                },
                UiState::Confirming => match key.code {
                    KeyCode::Char('y') | KeyCode::Enter => {
                        state.ui = UiState::Removing;
                        state.remove_idx = 0;
                    }
                    KeyCode::Char('n') | KeyCode::Esc => {
                        state.ui = UiState::Listing;
                    }
                    _ => {}
                },
                UiState::Done => return Ok(state.report),
                UiState::Removing => {}
            }
        }
    }
}

// ── rendering ─────────────────────────────────────────────────────────────────

fn render(f: &mut Frame, state: &mut State) {
    let size = f.area();
    if size.width == 0 || size.height == 0 {
        return;
    }

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(0),
            Constraint::Length(3),
        ])
        .split(size);

    let title_line = Line::from(vec![
        Span::styled(
            " klean ",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("globals — pacotes instalados globalmente"),
    ]);
    let header = Paragraph::new(title_line).block(Block::default().borders(Borders::ALL));
    f.render_widget(header, chunks[0]);

    match &state.ui {
        UiState::Scanning => render_scanning(f, chunks[1]),
        UiState::Listing => render_list(f, state, chunks[1]),
        UiState::Confirming => {
            render_list(f, state, chunks[1]);
            render_confirm_dialog(f, state, size);
        }
        UiState::Removing => render_removing(f, state, chunks[1]),
        UiState::Done => render_done(f, state, chunks[1]),
    }

    let footer_text = match state.ui {
        UiState::Scanning => " Escaneando… ",
        UiState::Listing => {
            " ↑↓/jk navegar  Espaço selecionar  a tudo/nada  Enter confirmar  q sair "
        }
        UiState::Confirming => " y/Enter confirmar remoção  n/Esc cancelar ",
        UiState::Removing => " Removendo… ",
        UiState::Done => " Qualquer tecla para sair ",
    };
    let footer = Paragraph::new(footer_text)
        .style(Style::default().fg(Color::DarkGray))
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(footer, chunks[2]);
}

fn render_scanning(f: &mut Frame, area: Rect) {
    let p = Paragraph::new("\n\n  🔍 Escaneando pacotes globais (npm, cargo, pip)…")
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(p, area);
}

fn render_list(f: &mut Frame, state: &mut State, area: Rect) {
    if state.packages.is_empty() {
        let p = Paragraph::new("\n\n  ✨ Nenhum pacote global encontrado.")
            .block(Block::default().borders(Borders::ALL));
        f.render_widget(p, area);
        return;
    }

    let header = Row::new(vec![
        Cell::from(""),
        Cell::from("Gerenciador").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("Pacote").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("Versão").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("Tamanho").style(Style::default().add_modifier(Modifier::BOLD)),
    ])
    .style(Style::default().fg(Color::Yellow))
    .height(1);

    let rows: Vec<Row> = state
        .packages
        .iter()
        .enumerate()
        .map(|(i, pkg)| {
            let is_sel = state.selected.contains(&i);
            let checkbox = if is_sel { "◉" } else { "○" };
            let mgr_color = match pkg.manager {
                Manager::Npm => Color::Red,
                Manager::Cargo => Color::Yellow,
                Manager::Pip => Color::Blue,
            };
            let row_style = if is_sel {
                Style::default().fg(Color::Green)
            } else {
                Style::default()
            };
            Row::new(vec![
                Cell::from(checkbox),
                Cell::from(format!("{} {}", pkg.manager.icon(), pkg.manager.label()))
                    .style(Style::default().fg(mgr_color)),
                Cell::from(pkg.name.as_str()),
                Cell::from(pkg.version.as_str()).style(Style::default().fg(Color::DarkGray)),
                Cell::from(pkg.size_string()).style(Style::default().fg(Color::Cyan)),
            ])
            .style(row_style)
        })
        .collect();

    let sel = state.selected.len();
    let title = if sel > 0 {
        format!(
            " {} pacotes  ({} selecionado(s)) ",
            state.packages.len(),
            sel
        )
    } else {
        format!(" {} pacotes ", state.packages.len())
    };

    let table = Table::new(
        rows,
        [
            Constraint::Length(2),
            Constraint::Length(12),
            Constraint::Min(24),
            Constraint::Length(14),
            Constraint::Length(10),
        ],
    )
    .header(header)
    .block(Block::default().borders(Borders::ALL).title(title))
    .highlight_style(Style::default().bg(Color::DarkGray));

    f.render_stateful_widget(table, area, &mut state.table);
}

fn render_confirm_dialog(f: &mut Frame, state: &State, area: Rect) {
    let count = state.selected.len();
    let freed: u64 = state
        .selected
        .iter()
        .filter_map(|&i| state.packages.get(i))
        .map(|p| p.size_bytes)
        .sum();
    let freed_str = if freed > 0 {
        format_size(freed, humansize::BINARY)
    } else {
        "tamanho desconhecido".to_string()
    };

    let dialog_w = 52u16.min(area.width.saturating_sub(4));
    let dialog_h = 7u16;
    let x = area.x + (area.width.saturating_sub(dialog_w)) / 2;
    let y = area.y + (area.height.saturating_sub(dialog_h)) / 2;
    let dialog_area = Rect::new(x, y, dialog_w, dialog_h);

    f.render_widget(Clear, dialog_area);

    let text = format!(
        "\n  Remover {} pacote(s)?\n  Espaço liberado: {}\n\n  [y/Enter] Confirmar    [n/Esc] Cancelar",
        count, freed_str
    );
    let dialog = Paragraph::new(text)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Confirmar remoção ")
                .border_style(Style::default().fg(Color::Red)),
        )
        .style(Style::default().fg(Color::White));
    f.render_widget(dialog, dialog_area);
}

fn render_removing(f: &mut Frame, state: &State, area: Rect) {
    let total = state.selected.len();
    let done = state.remove_idx.min(total);
    let percent = done
        .checked_div(total)
        .map(|v| (v * 100) as u16)
        .unwrap_or(100);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(0)])
        .split(area);

    let gauge = Gauge::default()
        .block(Block::default().borders(Borders::ALL).title(" Removendo "))
        .gauge_style(Style::default().fg(Color::Red))
        .percent(percent)
        .label(format!("{}/{}", done, total));
    f.render_widget(gauge, chunks[0]);

    let visible = chunks[1].height.saturating_sub(2) as usize;
    let log_lines: Vec<Line> = state
        .remove_log
        .iter()
        .rev()
        .take(visible)
        .map(|l| Line::from(l.as_str()))
        .collect();
    let log =
        Paragraph::new(log_lines).block(Block::default().borders(Borders::ALL).title(" Log "));
    f.render_widget(log, chunks[1]);
}

fn render_done(f: &mut Frame, state: &State, area: Rect) {
    let freed = if state.report.freed > 0 {
        format_size(state.report.freed, humansize::BINARY)
    } else {
        "—".to_string()
    };
    let text = format!(
        "\n\n  ✓ {} pacote(s) removido(s)  •  {} liberados\n\n  Pressione qualquer tecla para sair.",
        state.report.removed, freed
    );
    let p = Paragraph::new(text)
        .block(Block::default().borders(Borders::ALL).title(" Concluído "))
        .style(Style::default().fg(Color::Green));
    f.render_widget(p, area);
}
