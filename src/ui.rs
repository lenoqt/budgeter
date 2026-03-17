//! Ratatui rendering — draws every tab, popup, and the status bar.

use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{
        Block, Borders, Cell, Clear, Paragraph, Row, Table, TableState, Tabs, Wrap,
    },
};

use crate::app::{App, EditMode, FamilyField, ImportFocus, IncomeField, LoanField, OtherField, PersonalField, Popup, Tab};
use crate::model::IncomeScenario;

/// Get the display string for a cell — returns the edit buffer when actively editing that cell.
fn cell_display(app: &App, row_idx: usize, is_active_col: bool, raw: String) -> String {
    if row_idx == app.selected_row && is_active_col && app.edit_mode == EditMode::Editing {
        app.edit_buf.clone()
    } else {
        raw
    }
}

// ── Palette ───────────────────────────────────────────────────────────────────

const C_HEADER_BG: Color   = Color::Rgb(30, 50, 80);
const C_HEADER_FG: Color   = Color::Rgb(200, 220, 255);
const C_SELECT_BG: Color   = Color::Rgb(45, 80, 130);
const C_SELECT_FG: Color   = Color::White;
const C_EDIT_BG: Color     = Color::Rgb(80, 60, 20);
const C_EDIT_FG: Color     = Color::Rgb(255, 230, 100);
const C_TOTAL_FG: Color    = Color::Rgb(140, 220, 140);
const C_BALANCE_POS: Color = Color::Rgb(100, 220, 100);
const C_BALANCE_NEG: Color = Color::Rgb(220, 80,  80);
const C_DIM: Color         = Color::Rgb(120, 120, 140);
const C_TAB_SEL: Color     = Color::Rgb(255, 200, 60);
const C_ACCENT: Color      = Color::Rgb(80, 180, 230);
const C_BORDER: Color      = Color::Rgb(60, 80, 110);
const C_POPUP_BG: Color    = Color::Rgb(20, 25, 38);

// ── Formatters ────────────────────────────────────────────────────────────────

fn jpy(v: i64) -> String {
    if v == 0 {
        return "¥0".to_string();
    }
    let sign = if v < 0 { "-" } else { "" };
    let abs = v.unsigned_abs();
    // Add thousands separators.
    let s = abs.to_string();
    let mut result = String::new();
    for (i, ch) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            result.push(',');
        }
        result.push(ch);
    }
    let body: String = result.chars().rev().collect();
    format!("¥{}{}", sign, body)
}

fn pct(v: f64) -> String {
    format!("{:.0}%", v * 100.0)
}

fn balance_color(v: i64) -> Color {
    if v >= 0 { C_BALANCE_POS } else { C_BALANCE_NEG }
}

// ── Entry point ───────────────────────────────────────────────────────────────

pub fn draw(frame: &mut Frame, app: &mut App) {
    let area = frame.area();

    // Root layout: title bar | tabs bar | content | status bar
    let root = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // title
            Constraint::Length(2), // tabs
            Constraint::Min(0),    // content
            Constraint::Length(1), // status
        ])
        .split(area);

    draw_title(frame, app, root[0]);
    draw_tabs(frame, app, root[1]);
    draw_content(frame, app, root[2]);
    draw_status(frame, app, root[3]);

    // Popups drawn last (on top).
    match app.popup.clone() {
        Popup::MonthPicker               => draw_popup_month_picker(frame, app, area),
        Popup::NewMonth                  => draw_popup_new_month(frame, app, area),
        Popup::DeleteConfirm             => draw_popup_delete_confirm(frame, app, area),
        Popup::Help                      => draw_popup_help(frame, area),
        Popup::CategoryPicker { row }    => draw_popup_category_picker(frame, app, area, row),
        Popup::None                      => {}
    }
}

// ── Title bar ─────────────────────────────────────────────────────────────────

fn draw_title(frame: &mut Frame, app: &App, area: Rect) {
    let dirty_marker = if app.dirty { " [*]" } else { "" };
    let scenario_tag = format!(" │ Scenario: {}", app.scenario_label());
    let month_tag    = format!(" │ Month: {}", app.current_month);

    let line = Line::from(vec![
        Span::styled("  家族予算 Family Budget", Style::default().fg(C_TAB_SEL).add_modifier(Modifier::BOLD)),
        Span::styled(month_tag, Style::default().fg(C_ACCENT)),
        Span::styled(scenario_tag, Style::default().fg(C_DIM)),
        Span::styled(dirty_marker, Style::default().fg(C_BALANCE_NEG).add_modifier(Modifier::BOLD)),
    ]);

    let para = Paragraph::new(line)
        .style(Style::default().bg(Color::Rgb(15, 20, 32)));
    frame.render_widget(para, area);
}

// ── Tabs bar ──────────────────────────────────────────────────────────────────

fn draw_tabs(frame: &mut Frame, app: &App, area: Rect) {
    let titles: Vec<Line> = Tab::ALL
        .iter()
        .map(|t| Line::from(Span::styled(format!(" {} ", t.title()), Style::default().fg(C_HEADER_FG))))
        .collect();

    let tabs = Tabs::new(titles)
        .select(app.active_tab.index())
        .block(Block::default().borders(Borders::BOTTOM).border_style(Style::default().fg(C_BORDER)))
        .highlight_style(
            Style::default()
                .fg(C_TAB_SEL)
                .add_modifier(Modifier::BOLD)
                .bg(Color::Rgb(30, 40, 60)),
        )
        .divider(Span::styled("│", Style::default().fg(C_BORDER)));

    frame.render_widget(tabs, area);
}

// ── Content router ────────────────────────────────────────────────────────────

fn draw_content(frame: &mut Frame, app: &mut App, area: Rect) {
    match app.active_tab {
        Tab::Income           => draw_income(frame, app, area),
        Tab::Loans            => draw_loans(frame, app, area),
        Tab::PersonalExpenses => draw_personal(frame, app, area),
        Tab::FamilyExpenses   => draw_family(frame, app, area),
        Tab::OtherItems       => draw_other(frame, app, area),
        Tab::Summary          => draw_summary(frame, app, area),
        Tab::Spending         => draw_spending(frame, app, area),
        Tab::Import           => draw_import(frame, app, area),
    }
}

// ── Helper: build a styled header row ────────────────────────────────────────

fn header_row(cells: &[&str]) -> Row<'static> {
    let styled: Vec<Cell<'static>> = cells
        .iter()
        .map(|c| {
            Cell::from(c.to_string()).style(
                Style::default()
                    .fg(C_HEADER_FG)
                    .bg(C_HEADER_BG)
                    .add_modifier(Modifier::BOLD),
            )
        })
        .collect();
    Row::new(styled).height(1)
}

/// Return `(row_style, cell_style_for_col)` taking editing into account.
fn cell_style(app: &App, row_idx: usize, is_active_col: bool) -> Style {
    let selected = row_idx == app.selected_row;
    if selected && is_active_col && app.edit_mode == EditMode::Editing {
        Style::default().fg(C_EDIT_FG).bg(C_EDIT_BG).add_modifier(Modifier::BOLD)
    } else if selected && is_active_col {
        Style::default().fg(C_SELECT_FG).bg(C_SELECT_BG).add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
    } else if selected {
        Style::default().fg(C_SELECT_FG).bg(Color::Rgb(35, 55, 90))
    } else {
        Style::default().fg(Color::White)
    }
}

// ── Keybinding hint footer ────────────────────────────────────────────────────

fn hint_line(hints: &[(&str, &str)]) -> Line<'static> {
    let mut spans: Vec<Span<'static>> = Vec::new();
    for (i, (key, desc)) in hints.iter().enumerate() {
        if i > 0 {
            spans.push(Span::styled("  ", Style::default()));
        }
        spans.push(Span::styled(key.to_string(), Style::default().fg(C_TAB_SEL).add_modifier(Modifier::BOLD)));
        spans.push(Span::styled(format!(" {}", desc), Style::default().fg(C_DIM)));
    }
    Line::from(spans)
}

// ── Income tab ───────────────────────────────────────────────────────────────

fn draw_income(frame: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(2)])
        .split(area);

    let header = header_row(&["Member", "Income After Tax", "Parental Leave (~180d)", "Parental Leave (180d+)"]);

    let mut rows: Vec<Row> = Vec::new();
    for (i, member) in app.budget.income.members.iter().enumerate() {
        let col_name     = is_col(app, Tab::Income, 0);
        let col_after    = is_col(app, Tab::Income, 1);
        let col_pl_early = is_col(app, Tab::Income, 2);
        let col_pl_late  = is_col(app, Tab::Income, 3);

        let name_s  = cell_display(app, i, col_name,     member.name.clone());
        let after_s = cell_display(app, i, col_after,    jpy(member.income_after_tax));
        let early_s = cell_display(app, i, col_pl_early, jpy(member.parental_leave_early));
        let late_s  = cell_display(app, i, col_pl_late,  jpy(member.parental_leave_late));

        let cells = vec![
            Cell::from(name_s).style(cell_style(app, i, col_name)),
            Cell::from(after_s).style(cell_style(app, i, col_after)),
            Cell::from(early_s).style(cell_style(app, i, col_pl_early)),
            Cell::from(late_s).style(cell_style(app, i, col_pl_late)),
        ];
        rows.push(Row::new(cells).height(1));
    }

    // Total row
    let total_style = Style::default().fg(C_TOTAL_FG).add_modifier(Modifier::BOLD);
    rows.push(Row::new(vec![
        Cell::from("Total").style(total_style),
        Cell::from(jpy(app.budget.income.total_after_tax())).style(total_style),
        Cell::from(jpy(app.budget.income.total_pl_early())).style(total_style),
        Cell::from(jpy(app.budget.income.total_pl_late())).style(total_style),
    ]));

    let table = Table::new(
        rows,
        [
            Constraint::Percentage(25),
            Constraint::Percentage(25),
            Constraint::Percentage(25),
            Constraint::Percentage(25),
        ],
    )
    .header(header)
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(C_BORDER))
            .title(Span::styled(" Income ", Style::default().fg(C_ACCENT).add_modifier(Modifier::BOLD))),
    );

    let mut state = TableState::default().with_selected(Some(app.selected_row));
    frame.render_stateful_widget(table, chunks[0], &mut state);
    frame.render_widget(
        Paragraph::new(hint_line(&[
            ("←→", "move col"),
            ("↑↓", "move row"),
            ("Enter", "edit"),
            ("Tab", "next section"),
        ])),
        chunks[1],
    );
}

fn is_col(app: &App, tab: Tab, col_idx: usize) -> bool {
    if app.active_tab != tab { return false; }
    match tab {
        Tab::Income => {
            let idx = match app.income_field {
                IncomeField::Name     => 0,
                IncomeField::AfterTax => 1,
                IncomeField::PlEarly  => 2,
                IncomeField::PlLate   => 3,
            };
            idx == col_idx
        }
        Tab::Loans => {
            let idx = match app.loan_field {
                LoanField::Label    => 0,
                LoanField::Fraction => 1,
            };
            idx == col_idx
        }
        Tab::PersonalExpenses => {
            let idx = match app.personal_field {
                PersonalField::Label   => 0,
                PersonalField::AmountA => 1,
                PersonalField::AmountB => 2,
            };
            idx == col_idx
        }
        Tab::FamilyExpenses => {
            let idx = match app.family_field {
                FamilyField::Label   => 0,
                FamilyField::Total   => 1,
                FamilyField::AmountA => 2,
                FamilyField::AmountB => 3,
            };
            idx == col_idx
        }
        Tab::OtherItems => {
            let idx = match app.other_field {
                OtherField::Label        => 0,
                OtherField::AnnualAmount => 1,
                OtherField::Notes        => 2,
            };
            idx == col_idx
        }
        Tab::Summary  => false,
        Tab::Spending => false,
        Tab::Import   => false,
    }
}

// ── Loans tab ────────────────────────────────────────────────────────────────

fn draw_loans(frame: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(2)])
        .split(area);

    let income_total = app.budget.effective_income_total(app.scenario);
    let loan_total   = app.budget.loan_total(app.scenario);
    let balance      = app.budget.balance_after_loans(app.scenario);

    let header = header_row(&["Loan / Setting", "Value / %", "Monthly JPY"]);

    let mut rows: Vec<Row> = Vec::new();

    // Row 0 — income fraction
    {
        let r    = 0usize;
        let col0 = is_col(app, Tab::Loans, 0);
        let col1 = is_col(app, Tab::Loans, 1);
        let label_s = cell_display(app, r, col0, "% of monthly income".to_string());
        let frac_s  = cell_display(app, r, col1, pct(app.budget.loans.income_fraction));
        rows.push(Row::new(vec![
            Cell::from(label_s).style(cell_style(app, r, col0)),
            Cell::from(frac_s).style(cell_style(app, r, col1)),
            Cell::from(jpy(loan_total)).style(Style::default().fg(C_TOTAL_FG).add_modifier(Modifier::BOLD)),
        ]));
    }

    // Individual loan rows (r starts at 1)
    for (i, loan) in app.budget.loans.loans.iter().enumerate() {
        let r    = i + 1;
        let col0 = is_col(app, Tab::Loans, 0);
        let col1 = is_col(app, Tab::Loans, 1);
        let label_s   = cell_display(app, r, col0, loan.label.clone());
        let frac_s    = cell_display(app, r, col1, pct(loan.fraction));
        let monthly   = jpy(app.budget.loans.payment_for(loan, income_total));
        rows.push(Row::new(vec![
            Cell::from(label_s).style(cell_style(app, r, col0)),
            Cell::from(frac_s).style(cell_style(app, r, col1)),
            Cell::from(monthly).style(Style::default().fg(C_DIM)),
        ]));
    }

    // Totals
    let ts = Style::default().fg(C_TOTAL_FG).add_modifier(Modifier::BOLD);
    let bs = Style::default().fg(balance_color(balance)).add_modifier(Modifier::BOLD);
    rows.push(Row::new(vec![
        Cell::from("Total income").style(ts),
        Cell::from("").style(ts),
        Cell::from(jpy(income_total)).style(ts),
    ]));
    rows.push(Row::new(vec![
        Cell::from("Total loan payment").style(ts),
        Cell::from(""),
        Cell::from(jpy(loan_total)).style(ts),
    ]));
    rows.push(Row::new(vec![
        Cell::from("Balance after loans").style(bs),
        Cell::from(""),
        Cell::from(jpy(balance)).style(bs),
    ]));

    let table = Table::new(
        rows,
        [
            Constraint::Percentage(40),
            Constraint::Percentage(20),
            Constraint::Percentage(40),
        ],
    )
    .header(header)
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(C_BORDER))
            .title(Span::styled(" Loans ", Style::default().fg(C_ACCENT).add_modifier(Modifier::BOLD))),
    );

    let mut state = TableState::default().with_selected(Some(app.selected_row));
    frame.render_stateful_widget(table, chunks[0], &mut state);
    frame.render_widget(
        Paragraph::new(hint_line(&[
            ("←→", "move col"),
            ("↑↓", "move row"),
            ("Enter", "edit"),
            ("s", "cycle scenario"),
        ])),
        chunks[1],
    );
}

// ── Personal Expenses tab ─────────────────────────────────────────────────────

fn draw_personal(frame: &mut Frame, app: &App, area: Rect) {
    let name_a = app.budget.income.members.first().map(|m| m.name.clone()).unwrap_or_else(|| "A".into());
    let name_b = app.budget.income.members.get(1).map(|m| m.name.clone()).unwrap_or_else(|| "B".into());

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(2)])
        .split(area);

    let hdr_a = format!("{} (JPY)", name_a);
    let hdr_b = format!("{} (JPY)", name_b);
    let header = header_row(&["Item", &hdr_a, &hdr_b, "Total"]);

    let mut rows: Vec<Row> = Vec::new();
    for (i, item) in app.budget.personal_expenses.items.iter().enumerate() {
        let col0 = is_col(app, Tab::PersonalExpenses, 0);
        let col1 = is_col(app, Tab::PersonalExpenses, 1);
        let col2 = is_col(app, Tab::PersonalExpenses, 2);

        let label_s = cell_display(app, i, col0, item.label.clone());
        let amt_a_s = cell_display(app, i, col1, jpy(item.amount_a));
        let amt_b_s = cell_display(app, i, col2, jpy(item.amount_b));
        let total_s = jpy(item.total());

        rows.push(Row::new(vec![
            Cell::from(label_s).style(cell_style(app, i, col0)),
            Cell::from(amt_a_s).style(cell_style(app, i, col1)),
            Cell::from(amt_b_s).style(cell_style(app, i, col2)),
            Cell::from(total_s).style(Style::default().fg(C_DIM)),
        ]));
    }

    // Totals row
    let ts = Style::default().fg(C_TOTAL_FG).add_modifier(Modifier::BOLD);
    rows.push(Row::new(vec![
        Cell::from("TOTAL").style(ts),
        Cell::from(jpy(app.budget.personal_expenses.total_a())).style(ts),
        Cell::from(jpy(app.budget.personal_expenses.total_b())).style(ts),
        Cell::from(jpy(app.budget.personal_expenses.total())).style(ts),
    ]));

    let table = Table::new(
        rows,
        [
            Constraint::Percentage(34),
            Constraint::Percentage(22),
            Constraint::Percentage(22),
            Constraint::Percentage(22),
        ],
    )
    .header(header)
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(C_BORDER))
            .title(Span::styled(" Personal Expenses ", Style::default().fg(C_ACCENT).add_modifier(Modifier::BOLD))),
    );

    let mut state = TableState::default().with_selected(Some(app.selected_row));
    frame.render_stateful_widget(table, chunks[0], &mut state);
    frame.render_widget(
        Paragraph::new(hint_line(&[
            ("←→", "move col"),
            ("↑↓", "move row"),
            ("Enter", "edit"),
            ("a", "add row"),
            ("d", "delete row"),
        ])),
        chunks[1],
    );
}

// ── Family Expenses tab ───────────────────────────────────────────────────────

fn draw_family(frame: &mut Frame, app: &App, area: Rect) {
    let name_a = app.budget.income.members.first().map(|m| m.name.clone()).unwrap_or_else(|| "A".into());
    let name_b = app.budget.income.members.get(1).map(|m| m.name.clone()).unwrap_or_else(|| "B".into());

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(2)])
        .split(area);

    let hdr_a = format!("{} share", name_a);
    let hdr_b = format!("{} share", name_b);
    let header = header_row(&["Item", "Total", &hdr_a, &hdr_b]);

    let mut rows: Vec<Row> = Vec::new();
    for (i, item) in app.budget.family_expenses.items.iter().enumerate() {
        let col0 = is_col(app, Tab::FamilyExpenses, 0);
        let col1 = is_col(app, Tab::FamilyExpenses, 1);
        let col2 = is_col(app, Tab::FamilyExpenses, 2);
        let col3 = is_col(app, Tab::FamilyExpenses, 3);

        let label_s = cell_display(app, i, col0, item.label.clone());
        let total_s = cell_display(app, i, col1, jpy(item.total));
        let amt_a_s = cell_display(app, i, col2, jpy(item.amount_a));
        let amt_b_s = cell_display(app, i, col3, jpy(item.amount_b));

        rows.push(Row::new(vec![
            Cell::from(label_s).style(cell_style(app, i, col0)),
            Cell::from(total_s).style(cell_style(app, i, col1)),
            Cell::from(amt_a_s).style(cell_style(app, i, col2)),
            Cell::from(amt_b_s).style(cell_style(app, i, col3)),
        ]));
    }

    let ts = Style::default().fg(C_TOTAL_FG).add_modifier(Modifier::BOLD);
    rows.push(Row::new(vec![
        Cell::from("TOTAL").style(ts),
        Cell::from(jpy(app.budget.family_expenses.total())).style(ts),
        Cell::from(jpy(app.budget.family_expenses.total_a())).style(ts),
        Cell::from(jpy(app.budget.family_expenses.total_b())).style(ts),
    ]));

    let table = Table::new(
        rows,
        [
            Constraint::Percentage(34),
            Constraint::Percentage(22),
            Constraint::Percentage(22),
            Constraint::Percentage(22),
        ],
    )
    .header(header)
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(C_BORDER))
            .title(Span::styled(" Family Expenses ", Style::default().fg(C_ACCENT).add_modifier(Modifier::BOLD))),
    );

    let mut state = TableState::default().with_selected(Some(app.selected_row));
    frame.render_stateful_widget(table, chunks[0], &mut state);
    frame.render_widget(
        Paragraph::new(hint_line(&[
            ("←→", "move col"),
            ("↑↓", "move row"),
            ("Enter", "edit"),
            ("a", "add row"),
            ("d", "delete row"),
            ("Tab", "next section"),
        ])),
        chunks[1],
    );
}

// ── Other Items tab ───────────────────────────────────────────────────────────

fn draw_other(frame: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(2)])
        .split(area);

    let header = header_row(&["Item", "Annual Amount", "Monthly Equiv.", "Notes"]);

    let mut rows: Vec<Row> = Vec::new();
    for (i, item) in app.budget.other_items.items.iter().enumerate() {
        let col0 = is_col(app, Tab::OtherItems, 0);
        let col1 = is_col(app, Tab::OtherItems, 1);
        let col2 = is_col(app, Tab::OtherItems, 2);

        let label_s  = cell_display(app, i, col0, item.label.clone());
        let annual_s = cell_display(app, i, col1, jpy(item.annual_amount));
        let monthly  = jpy(item.monthly_equivalent());
        let notes_s  = cell_display(app, i, col2, item.notes.clone());

        rows.push(Row::new(vec![
            Cell::from(label_s).style(cell_style(app, i, col0)),
            Cell::from(annual_s).style(cell_style(app, i, col1)),
            Cell::from(monthly).style(Style::default().fg(C_DIM)),
            Cell::from(notes_s).style(cell_style(app, i, col2)),
        ]));
    }

    let ts = Style::default().fg(C_TOTAL_FG).add_modifier(Modifier::BOLD);
    rows.push(Row::new(vec![
        Cell::from("TOTAL annual").style(ts),
        Cell::from(jpy(app.budget.other_items.total_annual())).style(ts),
        Cell::from(jpy(app.budget.other_items.total_monthly_equiv())).style(ts),
        Cell::from("").style(ts),
    ]));

    let table = Table::new(
        rows,
        [
            Constraint::Percentage(28),
            Constraint::Percentage(22),
            Constraint::Percentage(20),
            Constraint::Percentage(30),
        ],
    )
    .header(header)
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(C_BORDER))
            .title(Span::styled(" Other / Annual Items ", Style::default().fg(C_ACCENT).add_modifier(Modifier::BOLD))),
    );

    let mut state = TableState::default().with_selected(Some(app.selected_row));
    frame.render_stateful_widget(table, chunks[0], &mut state);
    frame.render_widget(
        Paragraph::new(hint_line(&[
            ("←→", "move col"),
            ("↑↓", "move row"),
            ("Enter", "edit"),
            ("a", "add row"),
            ("d", "delete row"),
        ])),
        chunks[1],
    );
}

// ── Summary tab ───────────────────────────────────────────────────────────────

fn draw_summary(frame: &mut Frame, app: &App, area: Rect) {
    let s = app.budget.summary(app.scenario);

    let name_a = app.budget.income.members.first().map(|m| m.name.clone()).unwrap_or_else(|| "A".into());
    let name_b = app.budget.income.members.get(1).map(|m| m.name.clone()).unwrap_or_else(|| "B".into());

    // Split into two horizontal panels: left summary table, right scenario selector.
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(65), Constraint::Percentage(35)])
        .split(area);

    // ── Left: summary table ────────────────────────────────────────────────────

    let header = header_row(&["Category", "Total", &name_a, &name_b]);

    fn summary_row(label: &'static str, total: i64, a: i64, b: i64, fg: Color) -> Row<'static> {
        let st = Style::default().fg(fg);
        Row::new(vec![
            Cell::from(label).style(st.add_modifier(Modifier::BOLD)),
            Cell::from(jpy(total)).style(st),
            Cell::from(jpy(a)).style(st),
            Cell::from(jpy(b)).style(st),
        ])
    }

    fn sep_row() -> Row<'static> {
        Row::new(vec![
            Cell::from("─────────────────"),
            Cell::from("────────────"),
            Cell::from("────────────"),
            Cell::from("────────────"),
        ])
        .style(Style::default().fg(C_BORDER))
    }

    let balance_col = balance_color(s.balance_total);

    let rows = vec![
        summary_row("Income",            s.income_total,          s.income_a,          s.income_b,          C_BALANCE_POS),
        sep_row(),
        summary_row("Loan payment",      app.budget.loan_total(app.scenario), 0, app.budget.loan_total(app.scenario), Color::Rgb(200, 140, 60)),
        sep_row(),
        summary_row("Family expenses",   s.family_expense_total,  s.family_expense_a,  s.family_expense_b,  Color::Rgb(180, 180, 80)),
        summary_row("Personal expenses", s.personal_expense_total,s.personal_expense_a,s.personal_expense_b,Color::Rgb(180, 180, 80)),
        sep_row(),
        summary_row("BALANCE",           s.balance_total,         s.balance_a,         s.balance_b,         balance_col),
    ];

    let table = Table::new(
        rows,
        [
            Constraint::Percentage(34),
            Constraint::Percentage(22),
            Constraint::Percentage(22),
            Constraint::Percentage(22),
        ],
    )
    .header(header)
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(C_BORDER))
            .title(Span::styled(
                format!(" Summary — {} ", app.scenario_label()),
                Style::default().fg(C_ACCENT).add_modifier(Modifier::BOLD),
            )),
    );

    frame.render_widget(table, chunks[0]);

    // ── Right: scenario + quick stats ─────────────────────────────────────────

    let right_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(8), Constraint::Min(0)])
        .split(chunks[1]);

    let scenarios = [
        IncomeScenario::Normal,
        IncomeScenario::ParentalLeaveEarly,
        IncomeScenario::ParentalLeaveLate,
    ];
    let scenario_labels = ["Normal", "Parental Leave (~180d)", "Parental Leave (180d+)"];

    let sc_lines: Vec<Line> = scenarios
        .iter()
        .zip(scenario_labels.iter())
        .map(|(sc, label)| {
            let active = *sc == app.scenario;
            let income = app.budget.effective_income_total(*sc);
            let bal    = app.budget.balance_final(*sc);
            let style  = if active {
                Style::default().fg(C_TAB_SEL).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(C_DIM)
            };
            let marker = if active { "▶ " } else { "  " };
            Line::from(vec![
                Span::styled(format!("{}{}", marker, label), style),
                Span::styled(
                    format!("  {} / bal {}", jpy(income), jpy(bal)),
                    Style::default().fg(if active { balance_color(bal) } else { C_DIM }),
                ),
            ])
        })
        .collect();

    let sc_para = Paragraph::new(sc_lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(C_BORDER))
                .title(Span::styled(" Scenarios (s) ", Style::default().fg(C_ACCENT))),
        )
        .wrap(Wrap { trim: false });
    frame.render_widget(sc_para, right_chunks[0]);

    // Quick-stats breakdown.
    let qs_lines = vec![
        Line::from(Span::styled("Annual other items:", Style::default().fg(C_DIM))),
        Line::from(Span::styled(
            format!("  Total:   {}", jpy(app.budget.other_items.total_annual())),
            Style::default().fg(Color::White),
        )),
        Line::from(Span::styled(
            format!("  Monthly: {}", jpy(app.budget.other_items.total_monthly_equiv())),
            Style::default().fg(C_DIM),
        )),
        Line::from(""),
        Line::from(Span::styled("Expense breakdown:", Style::default().fg(C_DIM))),
        Line::from(Span::styled(
            format!("  Family:   {}", jpy(s.family_expense_total)),
            Style::default().fg(Color::White),
        )),
        Line::from(Span::styled(
            format!("  Personal: {}", jpy(s.personal_expense_total)),
            Style::default().fg(Color::White),
        )),
        Line::from(Span::styled(
            format!("  Total:    {}", jpy(s.family_expense_total + s.personal_expense_total)),
            Style::default().fg(C_TOTAL_FG).add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::styled(
            format!("Loan rate:  {}", pct(app.budget.loans.income_fraction)),
            Style::default().fg(C_DIM),
        )),
        Line::from(Span::styled(
            format!("Loan total: {}", jpy(app.budget.loan_total(app.scenario))),
            Style::default().fg(Color::Rgb(200, 140, 60)),
        )),
    ];

    let qs_para = Paragraph::new(qs_lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(C_BORDER))
                .title(Span::styled(" Quick Stats ", Style::default().fg(C_ACCENT))),
        )
        .wrap(Wrap { trim: false });
    frame.render_widget(qs_para, right_chunks[1]);
}

// ── Spending tab ──────────────────────────────────────────────────────────────

fn draw_spending(frame: &mut Frame, app: &App, area: Rect) {
    let root = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(1)])
        .split(area);

    if app.spending_drill {
        draw_spending_drill(frame, app, root[0]);
    } else {
        draw_spending_overview(frame, app, root[0]);
    }

    let hint = if app.spending_drill {
        hint_line(&[("Esc", "back to categories"), ("d", "delete transaction"), ("e", "reassign category"), ("↑↓", "navigate")])
    } else {
        hint_line(&[("Enter/e", "drill into category"), ("↑↓", "navigate")])
    };
    frame.render_widget(Paragraph::new(hint), root[1]);
}

fn draw_spending_overview(frame: &mut Frame, app: &App, area: Rect) {
    let categories = app.budget.spending.active_categories();
    let total_spent = app.budget.spending.total_this_month();
    let uncategorized = app.budget.spending.total_uncategorized();

    // Split: summary panel on top, table below
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(4), Constraint::Min(0)])
        .split(area);

    // Summary panel
    let summary_line = Line::from(vec![
        Span::styled("  Total spent this month: ", Style::default().fg(C_DIM)),
        Span::styled(jpy(total_spent), Style::default().fg(C_BALANCE_NEG).add_modifier(Modifier::BOLD)),
        Span::styled("   Uncategorized: ", Style::default().fg(C_DIM)),
        Span::styled(jpy(uncategorized), Style::default().fg(if uncategorized > 0 { C_BALANCE_NEG } else { C_DIM })),
        Span::styled("   Transactions: ", Style::default().fg(C_DIM)),
        Span::styled(
            app.budget.spending.transactions.len().to_string(),
            Style::default().fg(C_ACCENT),
        ),
    ]);
    frame.render_widget(
        Paragraph::new(summary_line).block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(C_BORDER))
                .title(Span::styled(" Spending Overview ", Style::default().fg(C_TAB_SEL).add_modifier(Modifier::BOLD))),
        ),
        chunks[0],
    );

    if categories.is_empty() {
        frame.render_widget(
            Paragraph::new(Span::styled(
                "\n  No spending data. Use the Import tab to load card transactions.",
                Style::default().fg(C_DIM),
            )),
            chunks[1],
        );
        return;
    }

    // Category table
    let rows: Vec<Row> = categories
        .iter()
        .enumerate()
        .map(|(i, cat)| {
            let spent    = app.budget.spending.total_for_category(cat);
            let budgeted = app.budget.spending.categories
                .iter()
                .find(|c| c.name == *cat)
                .map(|c| c.budgeted)
                .unwrap_or(0);
            let remaining = if budgeted > 0 { budgeted - spent } else { 0 };
            let tx_count  = app.budget.spending.transactions.iter().filter(|t| t.category == *cat).count();

            // Bar: fill from 0..10 chars
            let bar = if budgeted > 0 {
                let ratio = (spent as f64 / budgeted as f64).min(1.0);
                let filled = (ratio * 10.0).round() as usize;
                format!("[{}{}]", "█".repeat(filled), "░".repeat(10 - filled))
            } else {
                "  (no budget) ".to_string()
            };

            let bar_color = if budgeted == 0 {
                C_DIM
            } else if spent > budgeted {
                C_BALANCE_NEG
            } else if spent as f64 > budgeted as f64 * 0.8 {
                Color::Rgb(220, 180, 60)
            } else {
                C_BALANCE_POS
            };

            let sel = i == app.spending_selected;
            let row_bg = if sel { Color::Rgb(35, 55, 90) } else { Color::Reset };

            let style     = Style::default().fg(Color::White).bg(row_bg);
            let amt_style = Style::default().fg(C_TOTAL_FG).bg(row_bg);
            let bar_style = Style::default().fg(bar_color).bg(row_bg);
            let rem_style = Style::default().fg(balance_color(remaining)).bg(row_bg);
            let sel_style = Style::default().fg(C_SELECT_FG).bg(if sel { C_SELECT_BG } else { Color::Reset });

            Row::new(vec![
                Cell::from(if sel { "> ".to_string() } else { "  ".to_string() }).style(sel_style),
                Cell::from(cat.clone()).style(style),
                Cell::from(format!("{}tx", tx_count)).style(style),
                Cell::from(jpy(spent)).style(amt_style),
                Cell::from(if budgeted > 0 { jpy(budgeted) } else { "-".to_string() }).style(style),
                Cell::from(if budgeted > 0 { jpy(remaining) } else { "-".to_string() }).style(rem_style),
                Cell::from(bar).style(bar_style),
            ])
        })
        .collect();

    let table = Table::new(
        rows,
        [
            Constraint::Length(2),
            Constraint::Min(20),
            Constraint::Length(5),
            Constraint::Length(12),
            Constraint::Length(12),
            Constraint::Length(12),
            Constraint::Length(14),
        ],
    )
    .header(header_row(&["", "Category", "Txs", "Spent", "Budgeted", "Remaining", "Usage"]))
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(C_BORDER))
            .title(Span::styled(" Categories ", Style::default().fg(C_TAB_SEL).add_modifier(Modifier::BOLD))),
    );

    let mut state = TableState::default().with_selected(Some(app.spending_selected));
    frame.render_stateful_widget(table, chunks[1], &mut state);
}

fn draw_spending_drill(frame: &mut Frame, app: &App, area: Rect) {
    let cat_name = app.spending_drilled_category_name()
        .unwrap_or_else(|| "(unknown)".to_string());
    let txs = app.spending_drilled_transactions();

    let spent    = app.budget.spending.total_for_category(&cat_name);
    let budgeted = app.budget.spending.categories
        .iter()
        .find(|c| c.name == cat_name)
        .map(|c| c.budgeted)
        .unwrap_or(0);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(4), Constraint::Min(0)])
        .split(area);

    // Summary header
    let summary = Line::from(vec![
        Span::styled(format!("  Category: "), Style::default().fg(C_DIM)),
        Span::styled(cat_name.clone(), Style::default().fg(C_TAB_SEL).add_modifier(Modifier::BOLD)),
        Span::styled("   Spent: ", Style::default().fg(C_DIM)),
        Span::styled(jpy(spent), Style::default().fg(C_BALANCE_NEG).add_modifier(Modifier::BOLD)),
        Span::styled("   Budgeted: ", Style::default().fg(C_DIM)),
        Span::styled(if budgeted > 0 { jpy(budgeted) } else { "-".to_string() }, Style::default().fg(C_TOTAL_FG)),
        Span::styled("   Remaining: ", Style::default().fg(C_DIM)),
        Span::styled(
            if budgeted > 0 { jpy(budgeted - spent) } else { "-".to_string() },
            Style::default().fg(balance_color(budgeted - spent)),
        ),
    ]);
    frame.render_widget(
        Paragraph::new(summary).block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(C_ACCENT))
                .title(Span::styled(" Category Detail ", Style::default().fg(C_TAB_SEL).add_modifier(Modifier::BOLD))),
        ),
        chunks[0],
    );

    // Transaction list
    let rows: Vec<Row> = txs
        .iter()
        .enumerate()
        .map(|(i, tx)| {
            let sel = i == app.spending_tx_selected;
            let row_bg = if sel { C_SELECT_BG } else { Color::Reset };
            let style  = Style::default().fg(Color::White).bg(row_bg);
            let amt_st = Style::default().fg(C_TOTAL_FG).bg(row_bg);
            let dim_st = Style::default().fg(C_DIM).bg(row_bg);
            let sel_st = Style::default().fg(C_SELECT_FG).bg(row_bg);

            // Truncate merchant to fit
            let merchant = if tx.merchant.len() > 28 {
                format!("{}…", &tx.merchant[..27])
            } else {
                tx.merchant.clone()
            };

            Row::new(vec![
                Cell::from(if sel { "> ".to_string() } else { "  ".to_string() }).style(sel_st),
                Cell::from(tx.date.clone()).style(dim_st),
                Cell::from(merchant).style(style),
                Cell::from(tx.cardholder.clone()).style(dim_st),
                Cell::from(jpy(tx.amount_this_month)).style(amt_st),
                Cell::from(if tx.fee != 0 { jpy(tx.fee) } else { "-".to_string() }).style(dim_st),
                Cell::from(tx.payment_method.clone()).style(dim_st),
            ])
        })
        .collect();

    let table = Table::new(
        rows,
        [
            Constraint::Length(2),
            Constraint::Length(11),
            Constraint::Min(20),
            Constraint::Length(8),
            Constraint::Length(11),
            Constraint::Length(10),
            Constraint::Min(14),
        ],
    )
    .header(header_row(&["", "Date", "Merchant", "Holder", "Amount", "Fee", "Method"]))
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(C_BORDER))
            .title(Span::styled(
                format!(" Transactions — {} ", cat_name),
                Style::default().fg(C_TAB_SEL).add_modifier(Modifier::BOLD),
            )),
    );

    let mut state = TableState::default().with_selected(Some(app.spending_tx_selected));
    frame.render_stateful_widget(table, chunks[1], &mut state);
}

// ── Import tab ────────────────────────────────────────────────────────────────

fn draw_import(frame: &mut Frame, app: &App, area: Rect) {
    // Layout: top controls bar | preview table | hint bar
    let root = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(7), Constraint::Min(0), Constraint::Length(1)])
        .split(area);

    draw_import_controls(frame, app, root[0]);
    draw_import_preview(frame, app, root[1]);

    let hint = hint_line(&[
        ("Tab/→←", "switch field"),
        ("Enter/e", "edit path / pick category / cycle provider"),
        ("P", "parse CSV"),
        ("C", "commit to budget"),
        ("X", "clear preview"),
        ("d", "discard row"),
    ]);
    frame.render_widget(Paragraph::new(hint), root[2]);
}

fn draw_import_controls(frame: &mut Frame, app: &App, area: Rect) {
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(0), Constraint::Length(36)])
        .margin(1)
        .split(area);

    // Left: file path input
    let path_focused = app.import_focus == ImportFocus::FilePath;
    let path_style = if path_focused && app.edit_mode == EditMode::Editing {
        Style::default().fg(C_EDIT_FG).bg(C_EDIT_BG).add_modifier(Modifier::BOLD)
    } else if path_focused {
        Style::default().fg(C_SELECT_FG).bg(C_SELECT_BG)
    } else {
        Style::default().fg(Color::White)
    };

    let path_text = if path_focused && app.edit_mode == EditMode::Editing {
        let buf = &app.edit_buf;
        let cursor = app.edit_cursor;
        let (before, after) = buf.split_at(cursor);
        format!("{}_{}",
            before,
            if after.is_empty() { "" } else { after }
        )
    } else if app.import_path_buf.is_empty() {
        "(enter file path — Tab to focus, Enter to edit)".to_string()
    } else {
        app.import_path_buf.clone()
    };

    frame.render_widget(
        Paragraph::new(Span::styled(path_text, path_style))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(if path_focused { C_ACCENT } else { C_BORDER }))
                    .title(Span::styled(" CSV File Path ", Style::default().fg(C_TAB_SEL).add_modifier(Modifier::BOLD))),
            ),
        cols[0],
    );

    // Right: provider selector
    let prov_focused = app.import_focus == ImportFocus::Provider;
    let prov_style = if prov_focused {
        Style::default().fg(C_SELECT_FG).bg(C_SELECT_BG).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::White)
    };

    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("  ", Style::default()),
            Span::styled(app.import_provider.label(), prov_style),
        ]))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(if prov_focused { C_ACCENT } else { C_BORDER }))
                .title(Span::styled(" Provider ", Style::default().fg(C_TAB_SEL).add_modifier(Modifier::BOLD))),
        ),
        cols[1],
    );

    // Outer border wrapping both
    frame.render_widget(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(C_BORDER))
            .title(Span::styled(" Import Settings ", Style::default().fg(C_TAB_SEL).add_modifier(Modifier::BOLD))),
        area,
    );
}

fn draw_import_preview(frame: &mut Frame, app: &App, area: Rect) {
    let preview = &app.import_preview;
    let tx_focused = app.import_focus == ImportFocus::TransactionList;

    if preview.is_empty() {
        frame.render_widget(
            Paragraph::new(Span::styled(
                "\n  No transactions loaded. Press P to parse the CSV file.",
                Style::default().fg(C_DIM),
            ))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(C_BORDER))
                    .title(Span::styled(" Preview ", Style::default().fg(C_TAB_SEL).add_modifier(Modifier::BOLD))),
            ),
            area,
        );
        return;
    }

    let categorized   = preview.iter().filter(|t| !t.category.is_empty()).count();
    let uncategorized = preview.len() - categorized;

    let rows: Vec<Row> = preview
        .iter()
        .enumerate()
        .map(|(i, tx)| {
            let sel = i == app.import_selected && tx_focused;
            let row_bg  = if i == app.import_selected { if tx_focused { C_SELECT_BG } else { Color::Rgb(35,55,90) } } else { Color::Reset };
            let cat_col = if tx.category.is_empty() {
                Style::default().fg(C_BALANCE_NEG).bg(row_bg).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(C_BALANCE_POS).bg(row_bg)
            };
            let style   = Style::default().fg(Color::White).bg(row_bg);
            let dim_st  = Style::default().fg(C_DIM).bg(row_bg);
            let amt_st  = Style::default().fg(C_TOTAL_FG).bg(row_bg);
            let sel_st  = Style::default().fg(C_SELECT_FG).bg(row_bg);

            let merchant = if tx.merchant.len() > 26 {
                format!("{}…", &tx.merchant[..25])
            } else {
                tx.merchant.clone()
            };
            let cat_text = if tx.category.is_empty() {
                "(unset — Enter to assign)".to_string()
            } else {
                tx.category.clone()
            };

            Row::new(vec![
                Cell::from(if sel { "> ".to_string() } else { format!("{:>3} ", i + 1) }).style(sel_st),
                Cell::from(tx.date.clone()).style(dim_st),
                Cell::from(merchant).style(style),
                Cell::from(tx.cardholder.clone()).style(dim_st),
                Cell::from(jpy(tx.amount_this_month)).style(amt_st),
                Cell::from(cat_text).style(cat_col),
            ])
        })
        .collect();

    let title = format!(
        " Preview — {} transactions ({} categorized, {} unset) ",
        preview.len(), categorized, uncategorized
    );

    let table = Table::new(
        rows,
        [
            Constraint::Length(4),
            Constraint::Length(11),
            Constraint::Min(20),
            Constraint::Length(8),
            Constraint::Length(11),
            Constraint::Min(22),
        ],
    )
    .header(header_row(&["#", "Date", "Merchant", "Holder", "Amount", "Category"]))
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(if tx_focused { C_ACCENT } else { C_BORDER }))
            .title(Span::styled(title, Style::default().fg(C_TAB_SEL).add_modifier(Modifier::BOLD))),
    );

    let mut state = TableState::default().with_selected(Some(app.import_selected));
    frame.render_stateful_widget(table, area, &mut state);
}

// ── Popup: category picker ────────────────────────────────────────────────────

fn draw_popup_category_picker(frame: &mut Frame, app: &App, area: Rect, row: usize) {
    let popup_area = centered_rect(55, 70, area);
    frame.render_widget(Clear, popup_area);
    frame.render_widget(
        Block::default().style(Style::default().bg(C_POPUP_BG)),
        popup_area,
    );

    let categories = app.budget.all_budget_categories();
    let merchant = app.import_preview
        .get(row)
        .map(|t| t.merchant.as_str())
        .unwrap_or("(unknown)");

    // Truncate merchant for title
    let merchant_short = if merchant.len() > 30 {
        format!("{}…", &merchant[..29])
    } else {
        merchant.to_string()
    };

    let title = Span::styled(
        format!(" Assign Category: {} ", merchant_short),
        Style::default().fg(C_TAB_SEL).add_modifier(Modifier::BOLD),
    );

    let items: Vec<Row> = categories
        .iter()
        .enumerate()
        .map(|(i, (name, budgeted))| {
            let sel = i == app.import_cat_cursor;
            let style = if sel {
                Style::default().fg(C_SELECT_FG).bg(C_SELECT_BG).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };
            let bgt_style = if sel {
                Style::default().fg(C_TOTAL_FG).bg(C_SELECT_BG)
            } else {
                Style::default().fg(C_DIM)
            };
            Row::new(vec![
                Cell::from(if sel { " > ".to_string() } else { "   ".to_string() }).style(style),
                Cell::from(name.clone()).style(style),
                Cell::from(if *budgeted > 0 { jpy(*budgeted) } else { "-".to_string() }).style(bgt_style),
            ])
        })
        .collect();

    let inner_area = Rect {
        x: popup_area.x,
        y: popup_area.y,
        width: popup_area.width,
        height: popup_area.height.saturating_sub(2),
    };

    let table = Table::new(
        items,
        [Constraint::Length(3), Constraint::Min(0), Constraint::Length(14)],
    )
    .header(header_row(&["", "Category", "Budgeted"]))
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(C_ACCENT))
            .title(title),
    );

    let mut state = TableState::default().with_selected(Some(app.import_cat_cursor));
    frame.render_stateful_widget(table, inner_area, &mut state);

    // Hint
    let hint_area = Rect {
        x: popup_area.x + 1,
        y: popup_area.y + popup_area.height - 2,
        width: popup_area.width - 2,
        height: 1,
    };
    frame.render_widget(
        Paragraph::new(hint_line(&[("Enter", "assign"), ("Esc", "cancel"), ("↑↓", "navigate")])),
        hint_area,
    );
}

// ── Status bar ────────────────────────────────────────────────────────────────

fn draw_status(frame: &mut Frame, app: &App, area: Rect) {
    let left_hints: Vec<Span> = if app.edit_mode == EditMode::Editing {
        vec![
            Span::styled("  EDITING", Style::default().fg(C_EDIT_FG).add_modifier(Modifier::BOLD | Modifier::RAPID_BLINK)),
            Span::styled("  Enter", Style::default().fg(C_TAB_SEL).add_modifier(Modifier::BOLD)),
            Span::styled(" confirm", Style::default().fg(C_DIM)),
            Span::styled("  Esc", Style::default().fg(C_TAB_SEL).add_modifier(Modifier::BOLD)),
            Span::styled(" cancel", Style::default().fg(C_DIM)),
        ]
    } else {
        vec![
            Span::styled("  q", Style::default().fg(C_TAB_SEL).add_modifier(Modifier::BOLD)),
            Span::styled(" quit", Style::default().fg(C_DIM)),
            Span::styled("  S", Style::default().fg(C_TAB_SEL).add_modifier(Modifier::BOLD)),
            Span::styled(" save", Style::default().fg(C_DIM)),
            Span::styled("  m", Style::default().fg(C_TAB_SEL).add_modifier(Modifier::BOLD)),
            Span::styled(" months", Style::default().fg(C_DIM)),
            Span::styled("  n", Style::default().fg(C_TAB_SEL).add_modifier(Modifier::BOLD)),
            Span::styled(" new month", Style::default().fg(C_DIM)),
            Span::styled("  ?", Style::default().fg(C_TAB_SEL).add_modifier(Modifier::BOLD)),
            Span::styled(" help", Style::default().fg(C_DIM)),
        ]
    };

    let status_right = if !app.status_msg.is_empty() {
        Span::styled(format!("{}  ", app.status_msg), Style::default().fg(C_ACCENT))
    } else {
        Span::styled(String::new(), Style::default())
    };

    let status_layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(0), Constraint::Length(40)])
        .split(area);

    frame.render_widget(
        Paragraph::new(Line::from(left_hints)).style(Style::default().bg(Color::Rgb(15, 20, 32))),
        status_layout[0],
    );
    frame.render_widget(
        Paragraph::new(Line::from(status_right))
            .alignment(Alignment::Right)
            .style(Style::default().bg(Color::Rgb(15, 20, 32))),
        status_layout[1],
    );
}

// ── Popup: month picker ───────────────────────────────────────────────────────

fn draw_popup_month_picker(frame: &mut Frame, app: &App, area: Rect) {
    let popup_area = centered_rect(50, 60, area);
    frame.render_widget(Clear, popup_area);
    frame.render_widget(
        Block::default().style(Style::default().bg(C_POPUP_BG)),
        popup_area,
    );

    let title = Span::styled(" Select Month ", Style::default().fg(C_TAB_SEL).add_modifier(Modifier::BOLD));

    let items: Vec<Row> = app.all_months
        .iter()
        .enumerate()
        .map(|(i, m)| {
            let marker = if m == &app.current_month { " ✓ " } else { "   " };
            let style = if i == app.popup_row {
                Style::default().fg(C_SELECT_FG).bg(C_SELECT_BG).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };
            Row::new(vec![
                Cell::from(marker.to_string()).style(style),
                Cell::from(m.clone()).style(style),
            ])
        })
        .collect();

    let table = Table::new(
        items,
        [Constraint::Length(4), Constraint::Min(0)],
    )
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(C_ACCENT))
            .title(title),
    );

    let mut state = TableState::default().with_selected(Some(app.popup_row));
    frame.render_stateful_widget(table, popup_area, &mut state);

    // Hint at bottom of popup
    let hint_area = Rect {
        x: popup_area.x + 1,
        y: popup_area.y + popup_area.height - 2,
        width: popup_area.width - 2,
        height: 1,
    };
    frame.render_widget(
        Paragraph::new(hint_line(&[("Enter", "load"), ("Del", "delete"), ("Esc", "close")])),
        hint_area,
    );
}

// ── Popup: new month ──────────────────────────────────────────────────────────

fn draw_popup_new_month(frame: &mut Frame, app: &App, area: Rect) {
    let popup_area = centered_rect(40, 20, area);
    frame.render_widget(Clear, popup_area);

    let inner = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Length(3), Constraint::Length(1)])
        .margin(1)
        .split(popup_area);

    frame.render_widget(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(C_ACCENT))
            .title(Span::styled(" New Month ", Style::default().fg(C_TAB_SEL).add_modifier(Modifier::BOLD)))
            .style(Style::default().bg(C_POPUP_BG)),
        popup_area,
    );

    frame.render_widget(
        Paragraph::new(Span::styled("Enter month (YYYY-MM):", Style::default().fg(C_DIM))),
        inner[0],
    );

    frame.render_widget(
        Paragraph::new(Span::styled(
            format!("{}_", app.new_month_buf),
            Style::default().fg(C_EDIT_FG).bg(C_EDIT_BG).add_modifier(Modifier::BOLD),
        ))
        .block(Block::default().borders(Borders::BOTTOM).border_style(Style::default().fg(C_ACCENT))),
        inner[1],
    );

    frame.render_widget(
        Paragraph::new(hint_line(&[("Enter", "create"), ("Esc", "cancel")])),
        inner[2],
    );
}

// ── Popup: delete confirm ─────────────────────────────────────────────────────

fn draw_popup_delete_confirm(frame: &mut Frame, app: &App, area: Rect) {
    let popup_area = centered_rect(44, 20, area);
    frame.render_widget(Clear, popup_area);

    let month = app.all_months.get(app.popup_row).cloned().unwrap_or_default();

    let inner = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(1)])
        .margin(2)
        .split(popup_area);

    frame.render_widget(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(C_BALANCE_NEG))
            .title(Span::styled(" Delete Month? ", Style::default().fg(C_BALANCE_NEG).add_modifier(Modifier::BOLD)))
            .style(Style::default().bg(C_POPUP_BG)),
        popup_area,
    );

    frame.render_widget(
        Paragraph::new(vec![
            Line::from(Span::styled("Delete all data for:", Style::default().fg(C_DIM))),
            Line::from(Span::styled(month.clone(), Style::default().fg(C_BALANCE_NEG).add_modifier(Modifier::BOLD))),
            Line::from(""),
            Line::from(Span::styled("This cannot be undone.", Style::default().fg(C_DIM))),
        ])
        .wrap(Wrap { trim: true }),
        inner[0],
    );

    frame.render_widget(
        Paragraph::new(hint_line(&[("y", "yes, delete"), ("n / Esc", "cancel")])),
        inner[1],
    );
}

// ── Popup: help ───────────────────────────────────────────────────────────────

fn draw_popup_help(frame: &mut Frame, area: Rect) {
    let popup_area = centered_rect(60, 70, area);
    frame.render_widget(Clear, popup_area);

    let lines: Vec<Line> = vec![
        Line::from(Span::styled("GLOBAL", Style::default().fg(C_ACCENT).add_modifier(Modifier::BOLD))),
        help_row("Tab / Shift-Tab", "Next / previous section"),
        help_row("q",               "Quit (auto-saves if unsaved)"),
        help_row("S",               "Save current month to parquet"),
        help_row("m",               "Open month picker"),
        help_row("n",               "New month (copies current values)"),
        help_row("s",               "Cycle income scenario"),
        help_row("?",               "Toggle this help"),
        Line::from(""),
        Line::from(Span::styled("NAVIGATION", Style::default().fg(C_ACCENT).add_modifier(Modifier::BOLD))),
        help_row("↑ / ↓",   "Move row"),
        help_row("← / →",   "Move column / focus"),
        help_row("Enter",    "Edit cell / enter drill-down"),
        help_row("Esc",      "Exit drill-down / cancel"),
        Line::from(""),
        Line::from(Span::styled("EDITING", Style::default().fg(C_ACCENT).add_modifier(Modifier::BOLD))),
        help_row("Enter",      "Commit value"),
        help_row("Esc",        "Cancel edit"),
        help_row("← / →",     "Move cursor"),
        help_row("Home / End", "Jump to start / end"),
        help_row("Backspace",  "Delete char before cursor"),
        help_row("Delete",     "Delete char at cursor"),
        Line::from(""),
        Line::from(Span::styled("ROWS", Style::default().fg(C_ACCENT).add_modifier(Modifier::BOLD))),
        help_row("a", "Add row (Personal / Family / Other tabs)"),
        help_row("d", "Delete selected row / transaction"),
        Line::from(""),
        Line::from(Span::styled("IMPORT TAB", Style::default().fg(C_ACCENT).add_modifier(Modifier::BOLD))),
        help_row("Tab / → ←",  "Switch between path / provider / list"),
        help_row("Enter / e",   "Edit path field; assign category to row; cycle provider"),
        help_row("P",           "Parse the CSV file (path must be set)"),
        help_row("C",           "Commit all previewed transactions to budget"),
        help_row("X",           "Clear the import preview without committing"),
        help_row("d",           "Discard the highlighted preview row"),
        Line::from(""),
        Line::from(Span::styled("SPENDING TAB", Style::default().fg(C_ACCENT).add_modifier(Modifier::BOLD))),
        help_row("Enter / e",   "Drill into category to view transactions"),
        help_row("Esc",         "Return to category overview"),
        help_row("e",           "Reassign category of selected transaction (drill view)"),
        help_row("d",           "Delete selected transaction (drill view)"),
        Line::from(""),
        Line::from(Span::styled("AMOUNTS", Style::default().fg(C_ACCENT).add_modifier(Modifier::BOLD))),
        help_row("", "Enter plain numbers, e.g. 50000"),
        help_row("", "¥ signs and commas are stripped automatically"),
        help_row("", "Percentages: enter 25 for 25%"),
    ];

    let para = Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(C_ACCENT))
                .title(Span::styled(" Help — Keyboard Shortcuts ", Style::default().fg(C_TAB_SEL).add_modifier(Modifier::BOLD)))
                .style(Style::default().bg(C_POPUP_BG)),
        )
        .wrap(Wrap { trim: false });

    frame.render_widget(para, popup_area);
}

fn help_row(key: &'static str, desc: &'static str) -> Line<'static> {
    Line::from(vec![
        Span::styled(
            format!("  {:20}", key),
            Style::default().fg(C_TAB_SEL).add_modifier(Modifier::BOLD),
        ),
        Span::styled(desc, Style::default().fg(Color::White)),
    ])
}

// ── Layout utilities ──────────────────────────────────────────────────────────

/// Return a centered rectangle of `percent_x` / `percent_y` of `r`.
fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
