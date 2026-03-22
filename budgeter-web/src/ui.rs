//! Ratatui rendering — draws every tab, popup, and the status bar.
//! Web version: uses ratatui 0.25 (matching webatui's dependency).
//! Import tab is replaced with a read-only info panel (no filesystem in WASM).
#![allow(dead_code)]

use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{
        Block, Borders, Cell, Clear, Paragraph, Row, Table, TableState, Tabs, Wrap,
    },
};

use budgeter_core::app::{App, EditMode, FamilyField, ImportFocus, IncomeField, OtherField, PersonalField, Popup, Tab};
use budgeter_core::model::IncomeScenario;

/// Truncate a string to at most `max_chars` Unicode scalar values, appending "…" if truncated.
/// This is safe for strings containing multi-byte characters (e.g. Japanese).
fn truncate_chars(s: &str, max_chars: usize) -> String {
    let mut chars = s.chars();
    let mut out = String::with_capacity(max_chars + 3);
    let mut count = 0;
    loop {
        match chars.next() {
            None => break,
            Some(c) => {
                count += 1;
                if count > max_chars {
                    out.push('…');
                    break;
                }
                out.push(c);
            }
        }
    }
    out
}

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
    let area = frame.size();

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
        Popup::LoanMemberPicker          => draw_popup_loan_member_picker(frame, app, area),
        Popup::ImportMemberPicker        => draw_popup_import_member_picker(frame, app, area),
        Popup::MemberPicker { row, is_spending } => draw_popup_member_picker(frame, app, area, row, is_spending),
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
        Tab::Charts           => draw_charts(frame, app, area),
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
        Tab::Loans => false, // loans uses row-based navigation per subsection
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
        Tab::Charts   => false,
    }
}

// ── Loans tab ────────────────────────────────────────────────────────────────

fn draw_loans(frame: &mut Frame, app: &App, area: Rect) {
    use budgeter_core::app::{LoanSection, DebtField};

    let income_total = app.budget.effective_income_total(app.scenario);
    let loan_total   = app.budget.loan_total(app.scenario);
    let balance      = app.budget.balance_after_loans(app.scenario);

    // Root layout: summary bar | three panels side-by-side | hint bar
    let root = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // summary
            Constraint::Min(0),    // panels
            Constraint::Length(1), // hint
        ])
        .split(area);

    // ── Summary bar ───────────────────────────────────────────────────────────
    let sum_line = Line::from(vec![
        Span::styled("  Income: ", Style::default().fg(C_DIM)),
        Span::styled(jpy(income_total), Style::default().fg(C_TOTAL_FG).add_modifier(Modifier::BOLD)),
        Span::styled("   Total loan payment: ", Style::default().fg(C_DIM)),
        Span::styled(jpy(loan_total), Style::default().fg(C_BALANCE_NEG).add_modifier(Modifier::BOLD)),
        Span::styled("   Balance after loans: ", Style::default().fg(C_DIM)),
        Span::styled(jpy(balance), Style::default().fg(balance_color(balance)).add_modifier(Modifier::BOLD)),
    ]);
    frame.render_widget(
        Paragraph::new(sum_line).block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(C_BORDER))
                .title(Span::styled(" Loans Summary ", Style::default().fg(C_TAB_SEL).add_modifier(Modifier::BOLD))),
        ),
        root[0],
    );

    // ── Three subsection panels ───────────────────────────────────────────────
    let panels = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(33),
            Constraint::Percentage(33),
            Constraint::Percentage(34),
        ])
        .split(root[1]);

    // Helper: border colour depending on whether this section is active.
    let section_border = |sec: LoanSection| {
        if app.loan_section == sec { C_ACCENT } else { C_BORDER }
    };
    let section_title_style = |sec: LoanSection| {
        if app.loan_section == sec {
            Style::default().fg(C_TAB_SEL).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(C_DIM)
        }
    };

    // ── Mortgage panel ────────────────────────────────────────────────────────
    {
        use budgeter_core::app::MortgageField;
        let m = &app.budget.loans.mortgage;
        let active = app.loan_section == LoanSection::Mortgage;

        // Each row is either an editable input or a computed read-only display.
        enum MRow {
            Input(MortgageField, &'static str, String),
            Computed(&'static str, String),
            Separator,
        }

        let rows_def: Vec<MRow> = vec![
            MRow::Input(MortgageField::Principal,    "Outstanding principal",     jpy(m.principal)),
            MRow::Input(MortgageField::InterestRate, "Annual rate % (年利)",      format!("{:.4}%", m.interest_rate * 100.0)),
            MRow::Input(MortgageField::RemainingMonths, "Remaining months",       format!("{} mo", m.remaining_months)),
            MRow::Input(MortgageField::MonthlyInsurance, "  + Insurance/fee (保証料)", jpy(m.monthly_insurance)),
            MRow::Input(MortgageField::ShareA,       "  Payment split (Enter to set)", {
                let name_a = app.budget.income.members.first().map(|m| m.name.as_str()).unwrap_or("A");
                let name_b = app.budget.income.members.get(1).map(|m| m.name.as_str()).unwrap_or("B");
                format!("{}: {:.0}%  /  {}: {:.0}%", name_a, m.share_a * 100.0, name_b, (1.0 - m.share_a) * 100.0)
            }),
            MRow::Input(MortgageField::Amortization, "Method (Enter to toggle)",  m.amortization.label().to_string()),
            MRow::Separator,
            MRow::Computed("  ↳ Monthly principal (元金)",  jpy(m.monthly_principal)),
            MRow::Computed("  ↳ Monthly interest  (利息)",  jpy(m.monthly_interest)),
            MRow::Separator,
            MRow::Computed("  = Monthly total (exact)",    jpy(m.monthly_total)),
            MRow::Computed("  = Monthly total (¥100 rounded)", jpy(m.rounded_total)),
            MRow::Computed("  = First payment  (w/ shortfall)", jpy(m.first_payment)),
        ];

        // Build the TableState selection index — only Input rows are selectable.
        let input_positions: Vec<usize> = rows_def.iter().enumerate()
            .filter_map(|(i, r)| if matches!(r, MRow::Input(..)) { Some(i) } else { None })
            .collect();
        let active_field_row = if active {
            MortgageField::ALL.iter().position(|f| *f == app.mortgage_field)
                .and_then(|fi| input_positions.get(fi).copied())
        } else {
            None
        };

        let rows: Vec<Row> = rows_def.iter().map(|row_def| {
            match row_def {
                MRow::Input(field, label, val) => {
                    let is_sel = active && app.mortgage_field == *field;
                    let editing = is_sel && app.edit_mode == budgeter_core::app::EditMode::Editing;
                    let display = if editing { app.edit_buf.clone() } else { val.clone() };
                    let is_toggle = matches!(field, MortgageField::Amortization);
                    let lbl_style = if is_sel {
                        Style::default().fg(C_SELECT_FG).bg(C_SELECT_BG).add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(Color::White)
                    };
                    let val_style = if editing {
                        Style::default().fg(C_EDIT_FG).bg(C_EDIT_BG).add_modifier(Modifier::BOLD)
                    } else if is_sel && is_toggle {
                        Style::default().fg(C_TAB_SEL).bg(C_SELECT_BG).add_modifier(Modifier::BOLD)
                    } else if is_sel {
                        Style::default().fg(C_SELECT_FG).bg(C_SELECT_BG).add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
                    } else if is_toggle {
                        Style::default().fg(C_TAB_SEL)
                    } else {
                        Style::default().fg(C_ACCENT)
                    };
                    Row::new(vec![
                        Cell::from(label.to_string()).style(lbl_style),
                        Cell::from(display).style(val_style),
                    ])
                }
                MRow::Computed(label, val) => {
                    Row::new(vec![
                        Cell::from(label.to_string()).style(Style::default().fg(C_DIM)),
                        Cell::from(val.clone()).style(Style::default().fg(C_TOTAL_FG).add_modifier(Modifier::BOLD)),
                    ])
                }
                MRow::Separator => {
                    Row::new(vec![
                        Cell::from("─────────────────────").style(Style::default().fg(Color::Rgb(50, 65, 90))),
                        Cell::from("────────────").style(Style::default().fg(Color::Rgb(50, 65, 90))),
                    ])
                }
            }
        }).collect();

        let table = Table::new(
            rows,
            [Constraint::Percentage(62), Constraint::Percentage(38)],
        )
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(section_border(LoanSection::Mortgage)))
                .title(Span::styled(
                    format!(" 🏠 Mortgage (exact {}/mo, ~{}/mo) ", jpy(m.monthly_total), jpy(m.rounded_total)),
                    section_title_style(LoanSection::Mortgage),
                )),
        );

        let mut state = TableState::default().with_selected(active_field_row);
        frame.render_stateful_widget(table, panels[0], &mut state);
    }

    // ── Car panel ─────────────────────────────────────────────────────────────
    {
        use budgeter_core::app::CarField;
        let c = &app.budget.loans.car;
        let active = app.loan_section == LoanSection::Car;

        enum CRow {
            Input(CarField, &'static str, String),
            Computed(&'static str, String),
            Separator,
        }

        let rows_def: Vec<CRow> = vec![
            CRow::Input(CarField::Principal,      "Outstanding principal",    jpy(c.principal)),
            CRow::Input(CarField::InterestRate,   "Annual rate % (年利)",     format!("{:.4}%", c.interest_rate * 100.0)),
            CRow::Input(CarField::RemainingMonths,"Remaining months",         format!("{} mo", c.remaining_months)),
            CRow::Input(CarField::ShareA,         "  Payment split (Enter to set)", {
                let name_a = app.budget.income.members.first().map(|m| m.name.as_str()).unwrap_or("A");
                let name_b = app.budget.income.members.get(1).map(|m| m.name.as_str()).unwrap_or("B");
                format!("{}: {:.0}%  /  {}: {:.0}%", name_a, c.share_a * 100.0, name_b, (1.0 - c.share_a) * 100.0)
            }),
            CRow::Input(CarField::Amortization,   "Method (Enter to toggle)", c.amortization.label().to_string()),
            CRow::Separator,
            CRow::Computed("  ↳ Monthly principal (元金)", jpy(c.monthly_principal)),
            CRow::Computed("  ↳ Monthly interest  (利息)", jpy(c.monthly_interest)),
            CRow::Separator,
            CRow::Computed("  = Monthly total (exact)",    jpy(c.monthly_total)),
            CRow::Computed("  = Monthly total (¥100 rounded)", jpy(c.rounded_total)),
            CRow::Computed("  = First payment  (w/ shortfall)", jpy(c.first_payment)),
        ];

        let input_positions: Vec<usize> = rows_def.iter().enumerate()
            .filter_map(|(i, r)| if matches!(r, CRow::Input(..)) { Some(i) } else { None })
            .collect();
        let active_field_row = if active {
            CarField::ALL.iter().position(|f| *f == app.car_field)
                .and_then(|fi| input_positions.get(fi).copied())
        } else {
            None
        };

        let rows: Vec<Row> = rows_def.iter().map(|row_def| {
            match row_def {
                CRow::Input(field, label, val) => {
                    let is_sel = active && app.car_field == *field;
                    let editing = is_sel && app.edit_mode == budgeter_core::app::EditMode::Editing;
                    let display = if editing { app.edit_buf.clone() } else { val.clone() };
                    let is_toggle = matches!(field, CarField::Amortization);
                    let lbl_style = if is_sel {
                        Style::default().fg(C_SELECT_FG).bg(C_SELECT_BG).add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(Color::White)
                    };
                    let val_style = if editing {
                        Style::default().fg(C_EDIT_FG).bg(C_EDIT_BG).add_modifier(Modifier::BOLD)
                    } else if is_sel && is_toggle {
                        Style::default().fg(C_TAB_SEL).bg(C_SELECT_BG).add_modifier(Modifier::BOLD)
                    } else if is_sel {
                        Style::default().fg(C_SELECT_FG).bg(C_SELECT_BG).add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
                    } else if is_toggle {
                        Style::default().fg(C_TAB_SEL)
                    } else {
                        Style::default().fg(C_ACCENT)
                    };
                    Row::new(vec![
                        Cell::from(label.to_string()).style(lbl_style),
                        Cell::from(display).style(val_style),
                    ])
                }
                CRow::Computed(label, val) => {
                    Row::new(vec![
                        Cell::from(label.to_string()).style(Style::default().fg(C_DIM)),
                        Cell::from(val.clone()).style(Style::default().fg(C_TOTAL_FG).add_modifier(Modifier::BOLD)),
                    ])
                }
                CRow::Separator => {
                    Row::new(vec![
                        Cell::from("─────────────────────").style(Style::default().fg(Color::Rgb(50, 65, 90))),
                        Cell::from("────────────").style(Style::default().fg(Color::Rgb(50, 65, 90))),
                    ])
                }
            }
        }).collect();

        let table = Table::new(
            rows,
            [Constraint::Percentage(62), Constraint::Percentage(38)],
        )
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(section_border(LoanSection::Car)))
                .title(Span::styled(
                    format!(" 🚗 Car Loan (exact {}/mo, ~{}/mo) ", jpy(c.monthly_total), jpy(c.rounded_total)),
                    section_title_style(LoanSection::Car),
                )),
        );

        let mut state = TableState::default().with_selected(active_field_row);
        frame.render_stateful_widget(table, panels[1], &mut state);
    }

    // ── Debts panel ───────────────────────────────────────────────────────────
    {
        let debts = &app.budget.loans.debts;
        let active = app.loan_section == LoanSection::Debts;
        let debts_total: i64 = debts.iter().map(|d| d.monthly_payment).sum();

        if debts.is_empty() {
            frame.render_widget(
                Paragraph::new(Span::styled(
                    "\n  No debts. Press 'a' to add one.",
                    Style::default().fg(C_DIM),
                ))
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(section_border(LoanSection::Debts)))
                        .title(Span::styled(
                            " 💳 Debts (¥0) ",
                            section_title_style(LoanSection::Debts),
                        )),
                ),
                panels[2],
            );
        } else {
            let name_a = app.budget.income.members.first().map(|m| m.name.clone()).unwrap_or_else(|| "A".into());
            let name_b = app.budget.income.members.get(1).map(|m| m.name.clone()).unwrap_or_else(|| "B".into());
            let split_header = format!("{}%/{}%", name_a, name_b);

            let col_labels = DebtField::ALL;
            let col_names_owned = ["Label".to_string(), "Principal".to_string(), "Monthly".to_string(), "Rate %".to_string(), "Months".to_string(), split_header];
            let col_names: Vec<&str> = col_names_owned.iter().map(|s| s.as_str()).collect();

            let rows: Vec<Row> = debts.iter().enumerate().map(|(i, d)| {
                let is_row_sel = active && app.selected_row == i;

                let vals = [
                    d.label.clone(),
                    jpy(d.principal),
                    jpy(d.monthly_payment),
                    format!("{:.4}%", d.interest_rate * 100.0),
                    format!("{} mo", d.remaining_months),
                    format!("{:.0}/{:.0}", d.share_a * 100.0, (1.0 - d.share_a) * 100.0),
                ];

                let cells: Vec<Cell> = col_labels.iter().zip(vals.iter()).map(|(field, val)| {
                    let is_sel = is_row_sel && app.debt_field == *field;
                    let editing = is_sel && app.edit_mode == budgeter_core::app::EditMode::Editing;
                    let display = if editing { app.edit_buf.clone() } else { val.clone() };
                    let style = if editing {
                        Style::default().fg(C_EDIT_FG).bg(C_EDIT_BG).add_modifier(Modifier::BOLD)
                    } else if is_sel {
                        Style::default().fg(C_SELECT_FG).bg(C_SELECT_BG).add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
                    } else if is_row_sel {
                        Style::default().fg(C_SELECT_FG).bg(Color::Rgb(35, 55, 90))
                    } else {
                        Style::default().fg(Color::White)
                    };
                    Cell::from(display).style(style)
                }).collect();

                Row::new(cells)
            }).collect();

            let table = Table::new(
                rows,
                [
                    Constraint::Min(12),
                    Constraint::Length(12),
                    Constraint::Length(11),
                    Constraint::Length(8),
                    Constraint::Length(8),
                    Constraint::Length(5),
                ],
            )
            .header(header_row(&col_names))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(section_border(LoanSection::Debts)))
                    .title(Span::styled(
                        format!(" 💳 Debts ({}/mo) ", jpy(debts_total)),
                        section_title_style(LoanSection::Debts),
                    )),
            );

            let sel = if active { Some(app.selected_row) } else { None };
            let mut state = TableState::default().with_selected(sel);
            frame.render_stateful_widget(table, panels[2], &mut state);
        }
    }

    // ── Hint bar ──────────────────────────────────────────────────────────────
    let hint = match app.loan_section {
        LoanSection::Debts => hint_line(&[
            ("Esc", "back to Mortgage"),
            ("↑↓", "select debt row"),
            ("←→", "move column (edge: switch panel)"),
            ("Enter/e", "edit cell"),
            ("a", "add debt"),
            ("d", "delete debt"),
        ]),
        _ => hint_line(&[
            ("←→", "switch panel"),
            ("↑↓", "move field"),
            ("Enter/e", "edit / cycle method"),
            ("s", "cycle scenario"),
        ]),
    };
    frame.render_widget(Paragraph::new(hint), root[2]);
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
        summary_row("Loan payment",      s.loan_payment, s.loan_a, s.loan_b, Color::Rgb(200, 140, 60)),
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
            format!("Mortgage:   {}", jpy(app.budget.loans.mortgage.monthly_total)),
            Style::default().fg(C_DIM),
        )),
        Line::from(Span::styled(
            format!("Car loan:   {}", jpy(app.budget.loans.car.monthly_total)),
            Style::default().fg(C_DIM),
        )),
        Line::from(Span::styled(
            format!("Debts:      {}", jpy(app.budget.loans.debts.iter().map(|d| d.monthly_payment).sum::<i64>())),
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
        hint_line(&[
            ("Esc", "back to categories"),
            ("d", "delete transaction"),
            ("e", "reassign category"),
            ("m", "reassign member"),
            ("↑↓", "navigate"),
        ])
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
            let merchant = truncate_chars(&tx.merchant, 28);

            let member_text = if tx.member.is_empty() {
                "-".to_string()
            } else {
                tx.member.clone()
            };
            let mem_style = if tx.member.is_empty() {
                Style::default().fg(C_DIM).bg(row_bg)
            } else {
                Style::default().fg(C_ACCENT).bg(row_bg)
            };

            Row::new(vec![
                Cell::from(if sel { "> ".to_string() } else { "  ".to_string() }).style(sel_st),
                Cell::from(tx.date.clone()).style(dim_st),
                Cell::from(merchant).style(style),
                Cell::from(tx.cardholder.clone()).style(dim_st),
                Cell::from(member_text).style(mem_style),
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
            Constraint::Length(10),
            Constraint::Length(11),
            Constraint::Length(10),
            Constraint::Min(14),
        ],
    )
    .header(header_row(&["", "Date", "Merchant", "Holder", "Member", "Amount", "Fee", "Method"]))
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

fn draw_import(frame: &mut Frame, _app: &App, area: Rect) {
    let lines = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled("  Web Mode — Read Only", Style::default().fg(C_TAB_SEL).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("  CSV import is not available in the browser.", Style::default().fg(Color::White)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("  To add transactions:", Style::default().fg(C_ACCENT).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(vec![
            Span::styled("    1. Run the terminal app:  ", Style::default().fg(C_DIM)),
            Span::styled("cargo run -p budgeter-tui", Style::default().fg(C_TAB_SEL)),
        ]),
        Line::from(vec![
            Span::styled("    2. Import your CSV on the Import tab.", Style::default().fg(C_DIM)),
        ]),
        Line::from(vec![
            Span::styled("    3. Save (S) — this writes ", Style::default().fg(C_DIM)),
            Span::styled("budget.json", Style::default().fg(C_TAB_SEL)),
            Span::styled(" automatically.", Style::default().fg(C_DIM)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("  To sync this web view:", Style::default().fg(C_ACCENT).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(vec![
            Span::styled("    Paste the contents of ", Style::default().fg(C_DIM)),
            Span::styled("budget.json", Style::default().fg(C_TAB_SEL)),
            Span::styled(" into localStorage key ", Style::default().fg(C_DIM)),
            Span::styled("\"budget_data\"", Style::default().fg(C_TAB_SEL)),
            Span::styled(" and reload.", Style::default().fg(C_DIM)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled(
                "    Open browser DevTools → Application → Local Storage → set budget_data",
                Style::default().fg(C_DIM),
            ),
        ]),
    ];

    frame.render_widget(
        Paragraph::new(lines)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(C_BORDER))
                    .title(Span::styled(
                        " Import  (web mode — read only) ",
                        Style::default().fg(C_ACCENT).add_modifier(Modifier::BOLD),
                    )),
            )
            .wrap(Wrap { trim: false }),
        area,
    );
}

fn draw_import_controls(frame: &mut Frame, app: &App, area: Rect) {
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(0), Constraint::Length(30), Constraint::Length(24)])
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

    // Right-most: member selector
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("  \u{1f464} ", Style::default().fg(C_DIM)),
            Span::styled(app.import_member_label(), Style::default().fg(C_ACCENT).add_modifier(Modifier::BOLD)),
        ]))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(C_BORDER))
                .title(Span::styled(" Assign to (W) ", Style::default().fg(C_TAB_SEL).add_modifier(Modifier::BOLD))),
        ),
        cols[2],
    );

    // Outer border wrapping all
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

            let merchant = truncate_chars(&tx.merchant, 26);
            let cat_text = if tx.category.is_empty() {
                "(unset — Enter to assign)".to_string()
            } else {
                tx.category.clone()
            };

            let member_text = if tx.member.is_empty() {
                "(m to set)".to_string()
            } else {
                tx.member.clone()
            };
            let mem_style = if tx.member.is_empty() {
                Style::default().fg(C_DIM).bg(row_bg)
            } else {
                Style::default().fg(C_ACCENT).bg(row_bg)
            };

            Row::new(vec![
                Cell::from(if sel { "> ".to_string() } else { format!("{:>3} ", i + 1) }).style(sel_st),
                Cell::from(tx.date.clone()).style(dim_st),
                Cell::from(merchant).style(style),
                Cell::from(tx.cardholder.clone()).style(dim_st),
                Cell::from(member_text).style(mem_style),
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
            Constraint::Length(10),
            Constraint::Length(11),
            Constraint::Min(22),
        ],
    )
    .header(header_row(&["#", "Date", "Merchant", "Holder", "Member", "Amount", "Category"]))
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(if tx_focused { C_ACCENT } else { C_BORDER }))
            .title(Span::styled(title, Style::default().fg(C_TAB_SEL).add_modifier(Modifier::BOLD))),
    );

    let mut state = TableState::default().with_selected(Some(app.import_selected));
    frame.render_stateful_widget(table, area, &mut state);
}

// ── Popup: per-transaction member picker ─────────────────────────────────────

fn draw_popup_member_picker(frame: &mut Frame, app: &App, area: Rect, _row: usize, is_spending: bool) {
    let names = app.member_picker_names();

    // Resolve the current member for this transaction so we can show a ✓ marker.
    let cur_member = if is_spending {
        if let Some(cat) = app.spending_drilled_category_name() {
            app.budget.spending.transactions
                .iter()
                .filter(|t| t.category == cat)
                .nth(app.spending_tx_selected)
                .map(|t| t.member.clone())
                .unwrap_or_default()
        } else {
            String::new()
        }
    } else {
        app.import_preview
            .get(app.import_selected)
            .map(|t| t.member.clone())
            .unwrap_or_default()
    };

    let context_label = if is_spending { "Transaction" } else { "Import row" };

    let popup_area = centered_rect(42, 14, area);
    frame.render_widget(Clear, popup_area);
    frame.render_widget(Block::default().style(Style::default().bg(C_POPUP_BG)), popup_area);

    let items: Vec<Row> = names.iter().enumerate().map(|(i, name)| {
        let sel = i == app.spending_member_cursor;
        let is_current = (cur_member.is_empty() && name == "Both / Shared")
            || (!cur_member.is_empty() && *name == cur_member);
        let row_style = if sel {
            Style::default().fg(C_SELECT_FG).bg(C_SELECT_BG).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::White)
        };
        let mark_style = if sel {
            Style::default().fg(C_TOTAL_FG).bg(C_SELECT_BG)
        } else {
            Style::default().fg(C_DIM)
        };
        Row::new(vec![
            Cell::from(if sel { " ▶ " } else { "   " }).style(row_style),
            Cell::from(name.clone()).style(row_style),
            Cell::from(if is_current { "✓" } else { " " }).style(mark_style),
        ])
    }).collect();

    let inner_area = Rect {
        x: popup_area.x,
        y: popup_area.y,
        width: popup_area.width,
        height: popup_area.height.saturating_sub(2),
    };

    let table = Table::new(
        items,
        [Constraint::Length(3), Constraint::Min(0), Constraint::Length(2)],
    )
    .header(header_row(&["", "Member", ""]))
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(C_ACCENT))
            .title(Span::styled(
                format!(" Assign {} to member ", context_label),
                Style::default().fg(C_TAB_SEL).add_modifier(Modifier::BOLD),
            )),
    );

    let mut state = TableState::default().with_selected(Some(app.spending_member_cursor));
    frame.render_stateful_widget(table, inner_area, &mut state);

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

// ── Popup: loan member picker ─────────────────────────────────────────────────

fn draw_popup_loan_member_picker(frame: &mut Frame, app: &App, area: Rect) {
    use budgeter_core::app::LoanSection;

    let popup_area = centered_rect(44, 16, area);
    frame.render_widget(Clear, popup_area);
    frame.render_widget(
        Block::default().style(Style::default().bg(C_POPUP_BG)),
        popup_area,
    );

    // Resolve the current share_a for the active loan section / debt row.
    let share_a = match app.loan_section {
        LoanSection::Mortgage => app.budget.loans.mortgage.share_a,
        LoanSection::Car      => app.budget.loans.car.share_a,
        LoanSection::Debts    => app.budget.loans.debts
            .get(app.selected_row)
            .map(|d| d.share_a)
            .unwrap_or(0.5),
    };

    let section_label = match app.loan_section {
        LoanSection::Mortgage => "Mortgage",
        LoanSection::Car      => "Car Loan",
        LoanSection::Debts    => "Debt",
    };

    let name_a = app.budget.income.members.first().map(|m| m.name.clone()).unwrap_or_else(|| "A".into());
    let name_b = app.budget.income.members.get(1).map(|m| m.name.clone()).unwrap_or_else(|| "B".into());

    let members = [
        (name_a, share_a),
        (name_b, 1.0 - share_a),
    ];

    let title = Span::styled(
        format!(" {} — Set payment share ", section_label),
        Style::default().fg(C_TAB_SEL).add_modifier(Modifier::BOLD),
    );

    let items: Vec<Row> = members
        .iter()
        .enumerate()
        .map(|(i, (name, pct))| {
            let sel = i == app.loan_share_picker_cursor;
            let row_style = if sel {
                Style::default().fg(C_SELECT_FG).bg(C_SELECT_BG).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };
            let pct_style = if sel {
                Style::default().fg(C_TOTAL_FG).bg(C_SELECT_BG).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(C_ACCENT)
            };
            Row::new(vec![
                Cell::from(if sel { " ▶ " } else { "   " }).style(row_style),
                Cell::from(name.clone()).style(row_style),
                Cell::from(format!("{:.0}%", pct * 100.0)).style(pct_style),
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
        [Constraint::Length(3), Constraint::Min(0), Constraint::Length(8)],
    )
    .header(header_row(&["", "Member", "Share"]))
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(C_ACCENT))
            .title(title),
    );

    let mut state = TableState::default().with_selected(Some(app.loan_share_picker_cursor));
    frame.render_stateful_widget(table, inner_area, &mut state);

    let hint_area = Rect {
        x: popup_area.x + 1,
        y: popup_area.y + popup_area.height - 2,
        width: popup_area.width - 2,
        height: 1,
    };
    frame.render_widget(
        Paragraph::new(hint_line(&[
            ("Enter", "set share for member"),
            ("Esc", "cancel"),
            ("↑↓", "navigate"),
        ])),
        hint_area,
    );
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
    let merchant_short = truncate_chars(merchant, 30);

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

// ── Popup: import member picker ───────────────────────────────────────────────

fn draw_popup_import_member_picker(frame: &mut Frame, app: &App, area: Rect) {
    let names: Vec<String> = {
        let mut v = vec!["Both / Shared".to_string()];
        v.extend(app.budget.income.members.iter().map(|m| m.name.clone()));
        v
    };

    let popup_area = centered_rect(40, 14, area);
    frame.render_widget(Clear, popup_area);
    frame.render_widget(Block::default().style(Style::default().bg(C_POPUP_BG)), popup_area);

    let items: Vec<Row> = names.iter().enumerate().map(|(i, name)| {
        let sel = i == app.import_member_cursor;
        let style = if sel {
            Style::default().fg(C_SELECT_FG).bg(C_SELECT_BG).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::White)
        };
        let cur_marker = if (app.import_member.is_empty() && i == 0) || app.import_member == *name {
            "✓"
        } else {
            " "
        };
        Row::new(vec![
            Cell::from(if sel { " ▶ ".to_string() } else { "   ".to_string() }).style(style),
            Cell::from(name.clone()).style(style),
            Cell::from(cur_marker.to_string()).style(style),
        ])
    }).collect();

    let inner_area = Rect {
        x: popup_area.x,
        y: popup_area.y,
        width: popup_area.width,
        height: popup_area.height.saturating_sub(2),
    };

    let table = Table::new(items, [Constraint::Length(3), Constraint::Min(0), Constraint::Length(2)])
        .header(header_row(&["", "Member", ""]))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(C_ACCENT))
                .title(Span::styled(
                    " Assign import to member ",
                    Style::default().fg(C_TAB_SEL).add_modifier(Modifier::BOLD),
                )),
        );

    let mut state = TableState::default().with_selected(Some(app.import_member_cursor));
    frame.render_stateful_widget(table, inner_area, &mut state);

    let hint_area = Rect {
        x: popup_area.x + 1,
        y: popup_area.y + popup_area.height.saturating_sub(2),
        width: popup_area.width.saturating_sub(2),
        height: 1,
    };
    frame.render_widget(
        Paragraph::new(hint_line(&[("Enter", "select"), ("Esc", "cancel"), ("\u{2191}\u{2193}", "navigate")])),
        hint_area,
    );
}

// ── Charts tab ────────────────────────────────────────────────────────────────

fn draw_charts(frame: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(45),
            Constraint::Percentage(30),
            Constraint::Percentage(25),
        ])
        .split(area);

    draw_chart_spending_bars(frame, app, chunks[0]);
    draw_chart_budget_vs_actual(frame, app, chunks[1]);
    draw_chart_member_split(frame, app, chunks[2]);
}

fn draw_chart_spending_bars(frame: &mut Frame, app: &App, area: Rect) {
    let categories = app.budget.spending.active_categories();

    if categories.is_empty() {
        frame.render_widget(
            Paragraph::new(Span::styled(
                "\n  No spending data. Import transactions first.",
                Style::default().fg(C_DIM),
            ))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(C_BORDER))
                    .title(Span::styled(
                        " Spending by Category ",
                        Style::default().fg(C_TAB_SEL).add_modifier(Modifier::BOLD),
                    )),
            ),
            area,
        );
        return;
    }

    let max_spent: i64 = categories
        .iter()
        .map(|c| app.budget.spending.total_for_category(c))
        .max()
        .unwrap_or(1)
        .max(1);

    let bar_width = (area.width as i64 - 30).max(10) as usize;

    let member_a_name: String = app
        .budget
        .income
        .members
        .first()
        .map(|m| m.name.clone())
        .unwrap_or_else(|| "A".to_string());

    let lines: Vec<Line> = categories
        .iter()
        .map(|cat| {
            let spent = app.budget.spending.total_for_category(cat);
            let ratio = (spent as f64 / max_spent as f64).min(1.0);
            let filled = (ratio * bar_width as f64).round() as usize;
            let empty = bar_width.saturating_sub(filled);

            let spent_a: i64 = app
                .budget
                .spending
                .transactions
                .iter()
                .filter(|t| {
                    t.category == *cat
                        && !t.member.is_empty()
                        && t.member == member_a_name
                })
                .map(|t| t.amount_this_month)
                .sum();

            let has_members = app
                .budget
                .spending
                .transactions
                .iter()
                .any(|t| t.category == *cat && !t.member.is_empty());

            let bar_color = if spent > max_spent * 8 / 10 {
                C_BALANCE_NEG
            } else if spent > max_spent / 2 {
                Color::Rgb(220, 180, 60)
            } else {
                C_BALANCE_POS
            };

            let label = if cat.len() > 16 {
                cat.chars().take(16).collect::<String>()
            } else {
                format!("{:<16}", cat)
            };

            let mut spans = vec![
                Span::styled(format!(" {}", label), Style::default().fg(Color::White)),
                Span::styled("\u{2588}".repeat(filled), Style::default().fg(bar_color)),
                Span::styled(
                    "\u{2591}".repeat(empty),
                    Style::default().fg(Color::Rgb(40, 40, 60)),
                ),
                Span::styled(
                    format!(" {}", jpy(spent)),
                    Style::default().fg(C_TOTAL_FG).add_modifier(Modifier::BOLD),
                ),
            ];
            if has_members {
                spans.push(Span::styled(
                    format!("  ({}: {})", member_a_name, jpy(spent_a)),
                    Style::default().fg(C_DIM),
                ));
            }
            Line::from(spans)
        })
        .collect();

    frame.render_widget(
        Paragraph::new(lines).block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(C_BORDER))
                .title(Span::styled(
                    " Spending by Category (proportional bars) ",
                    Style::default().fg(C_TAB_SEL).add_modifier(Modifier::BOLD),
                )),
        ),
        area,
    );
}

fn draw_chart_budget_vs_actual(frame: &mut Frame, app: &App, area: Rect) {
    let categories = app.budget.spending.active_categories();

    if categories.is_empty() {
        frame.render_widget(
            Paragraph::new(Span::styled("\n  No data.", Style::default().fg(C_DIM))).block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(C_BORDER))
                    .title(Span::styled(
                        " Budget vs Actual ",
                        Style::default().fg(C_TAB_SEL).add_modifier(Modifier::BOLD),
                    )),
            ),
            area,
        );
        return;
    }

    let bar_w = ((area.width as i64 - 28) / 2).max(4) as usize;

    let lines: Vec<Line> = categories
        .iter()
        .map(|cat| {
            let spent = app.budget.spending.total_for_category(cat);
            let budgeted = app
                .budget
                .spending
                .categories
                .iter()
                .find(|c| c.name == *cat)
                .map(|c| c.budgeted)
                .unwrap_or(0);
            let max_val = spent.max(budgeted).max(1);

            let spent_bars =
                ((spent as f64 / max_val as f64) * bar_w as f64).round() as usize;
            let budgeted_bars =
                ((budgeted as f64 / max_val as f64) * bar_w as f64).round() as usize;

            let over = budgeted > 0 && spent > budgeted;
            let spent_color = if over { C_BALANCE_NEG } else { C_BALANCE_POS };

            let label = if cat.len() > 14 {
                cat.chars().take(14).collect::<String>()
            } else {
                format!("{:<14}", cat)
            };

            let amount_str = if budgeted > 0 {
                format!("{}/{}", jpy(spent), jpy(budgeted))
            } else {
                jpy(spent)
            };

            Line::from(vec![
                Span::styled(format!(" {}", label), Style::default().fg(Color::White)),
                Span::styled("B:".to_string(), Style::default().fg(C_DIM)),
                Span::styled(
                    "\u{2593}".repeat(budgeted_bars),
                    Style::default().fg(Color::Rgb(80, 120, 200)),
                ),
                Span::styled(" ".repeat(bar_w.saturating_sub(budgeted_bars)), Style::default()),
                Span::styled(" S:".to_string(), Style::default().fg(C_DIM)),
                Span::styled(
                    "\u{2593}".repeat(spent_bars),
                    Style::default().fg(spent_color),
                ),
                Span::styled(
                    format!(" {}", amount_str),
                    Style::default().fg(C_DIM),
                ),
            ])
        })
        .collect();

    frame.render_widget(
        Paragraph::new(lines).block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(C_BORDER))
                .title(Span::styled(
                    " Budget (B) vs Actual Spent (S) ",
                    Style::default().fg(C_TAB_SEL).add_modifier(Modifier::BOLD),
                )),
        ),
        area,
    );
}

fn draw_chart_member_split(frame: &mut Frame, app: &App, area: Rect) {
    let s = app.budget.summary(app.scenario);
    let name_a = app
        .budget
        .income
        .members
        .first()
        .map(|m| m.name.clone())
        .unwrap_or_else(|| "A".to_string());
    let name_b = app
        .budget
        .income
        .members
        .get(1)
        .map(|m| m.name.clone())
        .unwrap_or_else(|| "B".to_string());

    let rows_data: &[(&str, i64, i64)] = &[
        ("Income",        s.income_a,          s.income_b),
        ("Loans",         s.loan_a,             s.loan_b),
        ("Family exp.",   s.family_expense_a,   s.family_expense_b),
        ("Personal exp.", s.personal_expense_a, s.personal_expense_b),
        ("BALANCE",       s.balance_a,          s.balance_b),
    ];

    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    let header_style = Style::default()
        .fg(C_TAB_SEL)
        .add_modifier(Modifier::BOLD);

    let max_val: i64 = rows_data
        .iter()
        .flat_map(|(_, a, b)| [a.abs(), b.abs()])
        .max()
        .unwrap_or(1)
        .max(1);

    let col_w = (area.width.saturating_sub(4) / 2) as usize;
    let bar_w = (col_w as i64 - 22).max(4) as usize;

    // Build lines for member A
    let mut lines_a: Vec<Line> = vec![Line::from(Span::styled(
        format!(" \u{2500}\u{2500} {} \u{2500}\u{2500} ", name_a),
        header_style,
    ))];
    for (label, val, _) in rows_data {
        let ratio = (val.abs() as f64 / max_val as f64).min(1.0);
        let filled = (ratio * bar_w as f64).round() as usize;
        let color = balance_color(*val);
        let label_s = if label.len() > 14 {
            label.chars().take(14).collect::<String>()
        } else {
            format!("{:<14}", label)
        };
        lines_a.push(Line::from(vec![
            Span::styled(format!(" {}", label_s), Style::default().fg(Color::White)),
            Span::styled(
                "\u{2588}".repeat(filled),
                Style::default().fg(color),
            ),
            Span::styled(
                format!(" {}", jpy(*val)),
                Style::default().fg(color).add_modifier(Modifier::BOLD),
            ),
        ]));
    }

    // Build lines for member B
    let mut lines_b: Vec<Line> = vec![Line::from(Span::styled(
        format!(" \u{2500}\u{2500} {} \u{2500}\u{2500} ", name_b),
        header_style,
    ))];
    for (label, _, val) in rows_data {
        let ratio = (val.abs() as f64 / max_val as f64).min(1.0);
        let filled = (ratio * bar_w as f64).round() as usize;
        let color = balance_color(*val);
        let label_s = if label.len() > 14 {
            label.chars().take(14).collect::<String>()
        } else {
            format!("{:<14}", label)
        };
        lines_b.push(Line::from(vec![
            Span::styled(format!(" {}", label_s), Style::default().fg(Color::White)),
            Span::styled(
                "\u{2588}".repeat(filled),
                Style::default().fg(color),
            ),
            Span::styled(
                format!(" {}", jpy(*val)),
                Style::default().fg(color).add_modifier(Modifier::BOLD),
            ),
        ]));
    }

    frame.render_widget(
        Paragraph::new(lines_a).block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(C_BORDER)),
        ),
        chunks[0],
    );
    frame.render_widget(
        Paragraph::new(lines_b).block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(C_BORDER)),
        ),
        chunks[1],
    );
}
