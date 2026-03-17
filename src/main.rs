//! Family Budget TUI — main entry point.
//! Handles terminal setup/teardown, the render loop, and all keyboard input.

mod app;
mod db;
mod import;
mod model;
mod ui;

use std::io;
use std::time::{Duration, Instant};

use anyhow::Result;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};

use app::{App, EditMode, ImportFocus, Popup, Tab};
use db::Db;
use model::Budget;

const DB_PATH: &str = "budget.parquet";
const TICK_MS: u64 = 50; // ~20 fps

fn main() -> Result<()> {
    // ── Load or initialise data ───────────────────────────────────────────────
    let db = Db::new(DB_PATH);
    let all_months = db.list_months()?;

    let budget = if let Some(last) = all_months.last() {
        db.load_month(last)?.unwrap_or_default()
    } else {
        Budget::default()
    };

    let mut app = App::new(budget, all_months);

    // ── Terminal setup ────────────────────────────────────────────────────────
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;

    // ── Event loop ────────────────────────────────────────────────────────────
    let result = run_loop(&mut terminal, &mut app, &db);

    // ── Teardown (always) ─────────────────────────────────────────────────────
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    result
}

fn run_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
    db: &Db,
) -> Result<()> {
    let tick = Duration::from_millis(TICK_MS);
    let mut last_tick = Instant::now();

    loop {
        // ── Draw ──────────────────────────────────────────────────────────────
        terminal.draw(|f| ui::draw(f, app))?;

        // ── Input ─────────────────────────────────────────────────────────────
        let timeout = tick
            .checked_sub(last_tick.elapsed())
            .unwrap_or(Duration::ZERO);

        if event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                let should_quit = handle_key(app, db, key.code, key.modifiers)?;
                if should_quit {
                    return Ok(());
                }
            }
        }

        // ── Tick ──────────────────────────────────────────────────────────────
        if last_tick.elapsed() >= tick {
            app.tick_status();
            last_tick = Instant::now();
        }
    }
}

/// Handle a single key event. Returns `true` when the app should quit.
fn handle_key(
    app: &mut App,
    db: &Db,
    code: KeyCode,
    modifiers: KeyModifiers,
) -> Result<bool> {
    // ── Popup mode ────────────────────────────────────────────────────────────
    match app.popup.clone() {
        Popup::MonthPicker => {
            return handle_month_picker(app, db, code);
        }
        Popup::NewMonth => {
            return handle_new_month(app, db, code);
        }
        Popup::DeleteConfirm => {
            return handle_delete_confirm(app, db, code);
        }
        Popup::Help => {
            if matches!(code, KeyCode::Esc | KeyCode::Char('?') | KeyCode::Char('q')) {
                app.popup = Popup::None;
            }
            return Ok(false);
        }
        Popup::CategoryPicker { row } => {
            return handle_category_picker(app, db, code, row);
        }
        Popup::None => {}
    }

    // ── Edit mode ─────────────────────────────────────────────────────────────
    if app.edit_mode == EditMode::Editing {
        return handle_editing(app, db, code);
    }

    // ── Normal mode ───────────────────────────────────────────────────────────
    handle_normal(app, db, code, modifiers)
}

// ── Normal-mode input ─────────────────────────────────────────────────────────

fn handle_normal(
    app: &mut App,
    db: &Db,
    code: KeyCode,
    modifiers: KeyModifiers,
) -> Result<bool> {
    // ── Tab-specific shortcuts first ──────────────────────────────────────────
    match app.active_tab {
        Tab::Import => {
            match code {
                // P — parse the CSV
                KeyCode::Char('p') | KeyCode::Char('P') => {
                    do_import_parse(app);
                    return Ok(false);
                }
                // C — commit preview to budget
                KeyCode::Char('c') | KeyCode::Char('C') => {
                    if !app.import_preview.is_empty() {
                        app.import_commit();
                        if let Err(e) = db.save(&app.budget) {
                            app.set_status(format!("Save error: {e}"));
                        } else {
                            app.set_status("Transactions committed and saved ✓");
                        }
                    } else {
                        app.set_status("Nothing to commit — parse a CSV first (P).");
                    }
                    return Ok(false);
                }
                // X — clear preview
                KeyCode::Char('x') | KeyCode::Char('X') => {
                    app.import_clear_preview();
                    app.set_status("Import preview cleared.");
                    return Ok(false);
                }
                _ => {}
            }
        }
        Tab::Spending => {
            match code {
                // Esc — exit drill-down view
                KeyCode::Esc => {
                    if app.spending_drill {
                        app.spending_drill = false;
                        return Ok(false);
                    }
                }
                // e — reassign category (drill view)
                KeyCode::Char('e') if app.spending_drill => {
                    if !app.spending_drilled_transactions().is_empty() {
                        app.import_cat_cursor = 0;
                        app.popup = Popup::CategoryPicker { row: usize::MAX }; // sentinel for spending reassign
                    }
                    return Ok(false);
                }
                _ => {}
            }
        }
        _ => {}
    }

    match code {
        // Quit
        KeyCode::Char('q') | KeyCode::Char('Q') => {
            if app.dirty {
                do_save(app, db)?;
            }
            return Ok(true);
        }

        // Save
        KeyCode::Char('S') | KeyCode::Char('s') if modifiers.contains(KeyModifiers::SHIFT) => {
            do_save(app, db)?;
        }
        KeyCode::Char('S') => {
            do_save(app, db)?;
        }

        // Cycle income scenario
        KeyCode::Char('s') => {
            app.cycle_scenario();
        }

        // Help
        KeyCode::Char('?') => {
            app.popup = Popup::Help;
        }

        // Month picker
        KeyCode::Char('m') => {
            app.popup_row = app
                .all_months
                .iter()
                .position(|m| m == &app.current_month)
                .unwrap_or(0);
            app.popup = Popup::MonthPicker;
        }

        // New month
        KeyCode::Char('n') => {
            app.new_month_buf = next_month_str(&app.current_month);
            app.popup = Popup::NewMonth;
        }

        // Tab navigation
        KeyCode::Tab => app.nav_tab_next(),
        KeyCode::BackTab => app.nav_tab_prev(),

        // Row navigation
        KeyCode::Down | KeyCode::Char('j') => app.nav_row_down(),
        KeyCode::Up   | KeyCode::Char('k') => app.nav_row_up(),

        // Column navigation
        KeyCode::Right | KeyCode::Char('l') => app.nav_col_next(),
        KeyCode::Left  | KeyCode::Char('h') => app.nav_col_prev(),

        // Begin editing / drill-in / open category picker
        KeyCode::Enter | KeyCode::Char('e') => {
            app.begin_edit();
        }

        // Add / delete row
        KeyCode::Char('a') => app.add_row(),
        KeyCode::Char('d') => app.delete_row(),

        _ => {}
    }
    Ok(false)
}

// ── Edit-mode input ───────────────────────────────────────────────────────────

fn handle_editing(app: &mut App, _db: &Db, code: KeyCode) -> Result<bool> {
    match code {
        KeyCode::Enter => {
            if let Err(msg) = app.commit_edit() {
                app.set_status(format!("⚠ {}", msg));
            }
        }
        KeyCode::Esc => app.cancel_edit(),

        KeyCode::Char(c) => app.edit_insert_char(c),
        KeyCode::Backspace => app.edit_backspace(),
        KeyCode::Delete => app.edit_delete(),

        KeyCode::Left  => app.edit_cursor_left(),
        KeyCode::Right => app.edit_cursor_right(),
        KeyCode::Home  => app.edit_cursor_home(),
        KeyCode::End   => app.edit_cursor_end(),

        _ => {}
    }
    Ok(false)
}

// ── Category-picker popup input ───────────────────────────────────────────────

fn handle_category_picker(
    app: &mut App,
    db: &Db,
    code: KeyCode,
    row: usize,
) -> Result<bool> {
    let cats = app.budget.all_budget_categories();
    let n = cats.len();

    match code {
        KeyCode::Esc => {
            app.popup = Popup::None;
        }

        KeyCode::Down | KeyCode::Char('j') => {
            if n > 0 {
                app.import_cat_cursor = (app.import_cat_cursor + 1).min(n - 1);
            }
        }
        KeyCode::Up | KeyCode::Char('k') => {
            if app.import_cat_cursor > 0 {
                app.import_cat_cursor -= 1;
            }
        }

        KeyCode::Enter => {
            if let Some((cat_name, _)) = cats.get(app.import_cat_cursor) {
                let cat = cat_name.clone();
                // sentinel: usize::MAX means we're reassigning a spending transaction
                if row == usize::MAX {
                    app.spending_reassign_category(cat);
                    if app.dirty {
                        if let Err(e) = db.save(&app.budget) {
                            app.set_status(format!("Save error: {e}"));
                        } else {
                            app.set_status("Transaction reassigned and saved ✓");
                        }
                    }
                } else {
                    app.import_assign_category_for_row(row, cat);
                    // After assigning, focus the list so user can pick next row
                    app.import_focus = ImportFocus::TransactionList;
                }
            }
        }

        _ => {}
    }
    Ok(false)
}

// ── Month-picker popup input ──────────────────────────────────────────────────

fn handle_month_picker(app: &mut App, db: &Db, code: KeyCode) -> Result<bool> {
    let n = app.all_months.len();
    match code {
        KeyCode::Esc | KeyCode::Char('q') => {
            app.popup = Popup::None;
        }

        KeyCode::Down | KeyCode::Char('j') => {
            if n > 0 {
                app.popup_row = (app.popup_row + 1).min(n - 1);
            }
        }
        KeyCode::Up | KeyCode::Char('k') => {
            if app.popup_row > 0 {
                app.popup_row -= 1;
            }
        }

        KeyCode::Enter => {
            if let Some(month) = app.all_months.get(app.popup_row).cloned() {
                if app.dirty {
                    do_save(app, db)?;
                }
                if let Some(b) = db.load_month(&month)? {
                    app.budget = b;
                    app.current_month = month;
                    app.dirty = false;
                    app.set_status("Month loaded.");
                }
            }
            app.popup = Popup::None;
        }

        KeyCode::Delete | KeyCode::Char('d') => {
            if !app.all_months.is_empty() {
                app.popup = Popup::DeleteConfirm;
            }
        }

        _ => {}
    }
    Ok(false)
}

// ── New-month popup input ─────────────────────────────────────────────────────

fn handle_new_month(app: &mut App, db: &Db, code: KeyCode) -> Result<bool> {
    match code {
        KeyCode::Esc => {
            app.new_month_buf.clear();
            app.popup = Popup::None;
        }
        KeyCode::Backspace => {
            app.new_month_buf.pop();
        }
        KeyCode::Char(c) if c.is_ascii_digit() || c == '-' => {
            if app.new_month_buf.len() < 7 {
                app.new_month_buf.push(c);
            }
        }
        KeyCode::Enter => {
            let month = app.new_month_buf.trim().to_string();
            if validate_month(&month) {
                if app.dirty {
                    do_save(app, db)?;
                }
                // Create new budget copying current values but with the new month.
                // Clear spending log for the new month — transactions are per-month.
                let mut new_budget = app.budget.clone();
                new_budget.month = month.clone();
                new_budget.spending = model::SpendingLog::default();
                db.save(&new_budget)?;

                app.all_months = db.list_months()?;
                app.budget = new_budget;
                app.current_month = month;
                app.dirty = false;
                app.set_status("New month created.");
            } else {
                app.set_status("Invalid month format — use YYYY-MM (e.g. 2025-07).");
            }
            app.new_month_buf.clear();
            app.popup = Popup::None;
        }
        _ => {}
    }
    Ok(false)
}

// ── Delete-confirm popup input ────────────────────────────────────────────────

fn handle_delete_confirm(app: &mut App, db: &Db, code: KeyCode) -> Result<bool> {
    match code {
        KeyCode::Char('y') | KeyCode::Char('Y') => {
            if let Some(month) = app.all_months.get(app.popup_row).cloned() {
                db.delete_month(&month)?;
                app.all_months = db.list_months()?;

                if app.current_month == month {
                    if let Some(next) = app.all_months.last().cloned() {
                        if let Some(b) = db.load_month(&next)? {
                            app.budget = b;
                            app.current_month = next;
                        }
                    } else {
                        app.budget = Budget::default();
                        app.current_month = app.budget.month.clone();
                    }
                    app.dirty = false;
                }

                if !app.all_months.is_empty() {
                    app.popup_row = app.popup_row.min(app.all_months.len() - 1);
                }

                app.set_status(format!("Deleted {}.", month));
            }
            app.popup = Popup::MonthPicker;
        }
        KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
            app.popup = Popup::MonthPicker;
        }
        _ => {}
    }
    Ok(false)
}

// ── Import action ─────────────────────────────────────────────────────────────

fn do_import_parse(app: &mut App) {
    let path = app.import_path_buf.trim().to_string();
    if path.is_empty() {
        app.set_status("Set a file path first (Tab to focus path, Enter to edit).");
        return;
    }
    let provider = app.import_provider;
    match import::parse_csv(&path, provider) {
        Ok(txs) => {
            let count = txs.len();
            app.import_preview = txs;
            app.import_selected = 0;
            app.import_focus = ImportFocus::TransactionList;
            app.set_status(format!("Parsed {count} transactions — assign categories then press C to commit."));
        }
        Err(e) => {
            app.set_status(format!("Parse error: {e}"));
        }
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn do_save(app: &mut App, db: &Db) -> Result<()> {
    db.save(&app.budget)?;
    app.all_months = db.list_months()?;
    app.dirty = false;
    app.set_status(format!("Saved {}  ✓", app.current_month));
    Ok(())
}

fn validate_month(s: &str) -> bool {
    if s.len() != 7 { return false; }
    let bytes = s.as_bytes();
    bytes[0..4].iter().all(|b| b.is_ascii_digit())
        && bytes[4] == b'-'
        && bytes[5..7].iter().all(|b| b.is_ascii_digit())
        && {
            let m: u32 = s[5..7].parse().unwrap_or(0);
            (1..=12).contains(&m)
        }
}

/// Compute the next calendar month string from "YYYY-MM".
fn next_month_str(current: &str) -> String {
    if current.len() != 7 {
        return chrono::Local::now().format("%Y-%m").to_string();
    }
    let year: i32  = current[0..4].parse().unwrap_or(2025);
    let month: u32 = current[5..7].parse().unwrap_or(1);
    if month == 12 {
        format!("{:04}-01", year + 1)
    } else {
        format!("{:04}-{:02}", year, month + 1)
    }
}
