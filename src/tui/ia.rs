//! Interactive TUI for managing AI environment (skills, CLIs, MCP servers).

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
    text::Line,
    widgets::{Block, Borders, Cell, Clear, Gauge, Paragraph, Row, Table, TableState},
    Frame, Terminal,
};
use std::cmp::Reverse;
use std::collections::BTreeSet;
use std::io::stdout;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::ia::{remove_item, scan_all, AIItem, AIType};

pub struct IAReport {
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
    items: Vec<AIItem>,
    selected: BTreeSet<usize>,
    table: TableState,
    ui: UiState,
    remove_log: Vec<String>,
    remove_idx: usize,
    report: IAReport,
    sort_by_size: bool,
    filter: FilterMode,
}

#[derive(Debug, Clone, PartialEq)]
enum FilterMode {
    All,
    Skills,
    CLIs,
    MCPServers,
    Duplicates,
}

impl State {
    fn new() -> Self {
        let mut table = TableState::default();
        table.select(Some(0));
        Self {
            items: Vec::new(),
            selected: BTreeSet::new(),
            table,
            ui: UiState::Scanning,
            remove_log: Vec::new(),
            remove_idx: 0,
            report: IAReport {
                removed: 0,
                freed: 0,
            },
            sort_by_size: false,
            filter: FilterMode::All,
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
        if self.selected.len() == self.filtered_items().len() {
            self.selected.clear();
        } else {
            self.selected = (0..self.filtered_items().len()).collect();
        }
    }

    fn select_all_duplicates(&mut self) {
        self.selected.clear();
        for (i, item) in self.filtered_items().iter().enumerate() {
            if item.is_duplicate {
                self.selected.insert(i);
            }
        }
    }

    fn move_cursor(&mut self, delta: i64) {
        let len = self.filtered_items().len();
        if len == 0 {
            return;
        }
        let cur = self.table.selected().unwrap_or(0) as i64;
        let next = ((cur + delta).rem_euclid(len as i64)) as usize;
        self.table.select(Some(next));
    }

    fn filtered_items(&self) -> Vec<AIItem> {
        let items = match self.filter {
            FilterMode::All => self.items.clone(),
            FilterMode::Skills => self
                .items
                .iter()
                .filter(|i| i.item_type == AIType::Skill)
                .cloned()
                .collect(),
            FilterMode::CLIs => self
                .items
                .iter()
                .filter(|i| i.item_type == AIType::CLI)
                .cloned()
                .collect(),
            FilterMode::MCPServers => self
                .items
                .iter()
                .filter(|i| i.item_type == AIType::MCPServer)
                .cloned()
                .collect(),
            FilterMode::Duplicates => self
                .items
                .iter()
                .filter(|i| i.is_duplicate)
                .cloned()
                .collect(),
        };
        items
    }

    fn selected_items(&self) -> Vec<AIItem> {
        let filtered = self.filtered_items();
        self.selected
            .iter()
            .filter_map(|&i| filtered.get(i).cloned())
            .collect()
    }

    fn sort_items(&mut self) {
        if self.sort_by_size {
            self.items.sort_by_key(|p| Reverse(p.size_bytes));
        } else {
            self.items.sort_by(|a, b| {
                a.item_type
                    .label()
                    .cmp(b.item_type.label())
                    .then(a.name.cmp(&b.name))
            });
        }
        self.selected.clear();
        self.table.select(if self.filtered_items().is_empty() {
            None
        } else {
            Some(0)
        });
    }

    fn cycle_filter(&mut self) {
        self.filter = match self.filter {
            FilterMode::All => FilterMode::Skills,
            FilterMode::Skills => FilterMode::CLIs,
            FilterMode::CLIs => FilterMode::MCPServers,
            FilterMode::MCPServers => FilterMode::Duplicates,
            FilterMode::Duplicates => FilterMode::All,
        };
        self.selected.clear();
        self.table.select(if self.filtered_items().is_empty() {
            None
        } else {
            Some(0)
        });
    }
}

struct TermGuard;

impl Drop for TermGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(stdout(), DisableMouseCapture, LeaveAlternateScreen);
    }
}

pub fn run_ia(dry_run: bool) -> Result<IAReport> {
    let scan_result: Arc<Mutex<Option<Vec<AIItem>>>> = Arc::new(Mutex::new(None));
    let scan_done = Arc::new(AtomicBool::new(false));
    {
        let result = Arc::clone(&scan_result);
        let done = Arc::clone(&scan_done);
        std::thread::spawn(move || {
            let items = scan_all();
            *result.lock().unwrap() = Some(items);
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
                if let Some(items) = guard.take() {
                    state.items = items;
                    state.sort_items();
                    state.ui = UiState::Listing;
                }
            }
        }

        // Step through removal without blocking
        if state.ui == UiState::Removing {
            let targets = state.selected_items();
            if state.remove_idx < targets.len() {
                let item = targets[state.remove_idx].clone();
                match remove_item(&item, dry_run) {
                    Ok(_) => {
                        state.report.removed += 1;
                        state.report.freed += item.size_bytes;
                        state.remove_log.push(format!(
                            "✓ {} {} ({})",
                            item.item_type.icon(),
                            item.name,
                            item.version
                        ));
                    }
                    Err(e) => {
                        state.remove_log.push(format!(
                            "✗ {} {} — {}",
                            item.item_type.icon(),
                            item.name,
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
                        return Ok(IAReport {
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
                    KeyCode::Char('d') => state.select_all_duplicates(),
                    KeyCode::Char('s') => {
                        state.sort_by_size = !state.sort_by_size;
                        state.sort_items();
                    }
                    KeyCode::Char('f') => state.cycle_filter(),
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
        .constraints([Constraint::Min(0), Constraint::Length(3)])
        .split(size);

    match &state.ui {
        UiState::Scanning => render_scanning(f, chunks[0]),
        UiState::Listing => render_list(f, state, chunks[0]),
        UiState::Confirming => {
            render_list(f, state, chunks[0]);
            render_confirm_dialog(f, state, size);
        }
        UiState::Removing => render_removing(f, state, chunks[0]),
        UiState::Done => render_done(f, state, chunks[0]),
    }

    let sort_label = if state.sort_by_size {
        "↕size"
    } else {
        "↕type"
    };

    let filter_label = match state.filter {
        FilterMode::All => "all",
        FilterMode::Skills => "skills",
        FilterMode::CLIs => "clis",
        FilterMode::MCPServers => "mcp",
        FilterMode::Duplicates => "dupes",
    };

    let footer_text = match state.ui {
        UiState::Scanning => " Scanning AI environment… ".to_string(),
        UiState::Listing => format!(
            " ↑↓/jk  Space select  a all  d dupes  {} s  filter:{} f  Enter remove  q quit ",
            sort_label, filter_label
        ),
        UiState::Confirming => " y/Enter confirm removal  n/Esc cancel ".to_string(),
        UiState::Removing => " Removing… ".to_string(),
        UiState::Done => " Press any key to exit ".to_string(),
    };

    let footer = Paragraph::new(footer_text)
        .style(Style::default().fg(Color::DarkGray))
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(footer, chunks[1]);
}

fn render_scanning(f: &mut Frame, area: Rect) {
    let p = Paragraph::new("\n\n  🔍 Scanning AI environment (skills, CLIs, MCP servers)…")
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(p, area);
}

fn render_list(f: &mut Frame, state: &mut State, area: Rect) {
    let filtered = state.filtered_items();

    if filtered.is_empty() {
        let p = Paragraph::new("\n\n  ✨ No AI items found.")
            .block(Block::default().borders(Borders::ALL));
        f.render_widget(p, area);
        return;
    }

    let header = Row::new(vec![
        Cell::from(" "),
        Cell::from("ARTIFACT").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from(Line::from("SIZE").alignment(ratatui::layout::Alignment::Right))
            .style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from(""),
        Cell::from("TYPE / VERSION").style(Style::default().add_modifier(Modifier::BOLD)),
    ])
    .style(Style::default().fg(Color::Yellow))
    .height(1);

    let rows: Vec<Row> = filtered
        .iter()
        .enumerate()
        .map(|(i, item)| {
            let is_sel = state.selected.contains(&i);
            let checkbox = if is_sel { "◉" } else { "○" };
            let type_color = match item.item_type {
                AIType::Skill => Color::Cyan,
                AIType::CLI => Color::Green,
                AIType::MCPServer => Color::Magenta,
            };
            let mut row_style = if is_sel {
                Style::default().fg(Color::Green)
            } else {
                Style::default()
            };

            let name_display = if item.is_duplicate {
                format!("⚠ {}", item.name)
            } else {
                item.name.clone()
            };

            if item.is_duplicate {
                row_style = row_style.fg(Color::Yellow);
            }

            let type_version = format!(
                "{} {} {}",
                item.item_type.icon(),
                item.item_type.label(),
                item.version
            );

            Row::new(vec![
                Cell::from(checkbox),
                Cell::from(name_display),
                Cell::from(
                    Line::from(item.size_string()).alignment(ratatui::layout::Alignment::Right),
                )
                .style(Style::default().fg(Color::Cyan)),
                Cell::from(""),
                Cell::from(type_version).style(Style::default().fg(type_color)),
            ])
            .style(row_style)
        })
        .collect();

    let sel = state.selected.len();
    let dup_count = filtered.iter().filter(|i| i.is_duplicate).count();
    let mut title_parts = vec![format!(" ia — {} items", filtered.len())];
    if sel > 0 {
        title_parts.push(format!("  ({} selected)", sel));
    }
    if dup_count > 0 {
        title_parts.push(format!("  ⚠ {} duplicates", dup_count));
    }
    let title = title_parts.join("");

    let table = Table::new(
        rows,
        [
            Constraint::Length(2),  // checkbox
            Constraint::Min(16),    // ARTIFACT/PACKAGE
            Constraint::Length(12), // SIZE (right-aligned)
            Constraint::Length(8),  // spacer
            Constraint::Min(20),    // TYPE / VERSION
        ],
    )
    .header(header)
    .block(Block::default().borders(Borders::ALL).title(title))
    .highlight_style(Style::default().bg(Color::DarkGray));

    f.render_stateful_widget(table, area, &mut state.table);
}

fn render_confirm_dialog(f: &mut Frame, state: &State, area: Rect) {
    let count = state.selected.len();
    let freed: u64 = state.selected_items().iter().map(|i| i.size_bytes).sum();
    let freed_str = if freed > 0 {
        format_size(freed, humansize::BINARY)
    } else {
        "unknown size".to_string()
    };

    let dialog_w = 52u16.min(area.width.saturating_sub(4));
    let dialog_h = 7u16;
    let x = area.x + (area.width.saturating_sub(dialog_w)) / 2;
    let y = area.y + (area.height.saturating_sub(dialog_h)) / 2;
    let dialog_area = Rect::new(x, y, dialog_w, dialog_h);

    f.render_widget(Clear, dialog_area);

    let text = format!(
        "\n  Remove {} AI item(s)?\n  Space freed: {}\n\n  [y/Enter] Confirm    [n/Esc] Cancel",
        count, freed_str
    );
    let dialog = Paragraph::new(text)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Confirm removal ")
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
        .block(Block::default().borders(Borders::ALL).title(" Removing "))
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
        "\n\n  ✓ {} AI item(s) removed  •  {} freed\n\n  Press any key to exit.",
        state.report.removed, freed
    );
    let p = Paragraph::new(text)
        .block(Block::default().borders(Borders::ALL).title(" Done "))
        .style(Style::default().fg(Color::Green));
    f.render_widget(p, area);
}
