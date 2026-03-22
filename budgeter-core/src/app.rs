//! Application state machine — tabs, navigation, editing, and business logic.

use crate::model::{
    Budget, CardProvider, Debt, FamilyExpenseItem, IncomeScenario, OtherItem, PersonalExpenseItem,
    Transaction,
};

// ── Tabs ──────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Income,
    Loans,
    PersonalExpenses,
    FamilyExpenses,
    OtherItems,
    Summary,
    Spending,
    Import,
    Charts,
}

impl Tab {
    pub const ALL: &'static [Tab] = &[
        Tab::Income,
        Tab::Loans,
        Tab::PersonalExpenses,
        Tab::FamilyExpenses,
        Tab::OtherItems,
        Tab::Summary,
        Tab::Spending,
        Tab::Import,
        Tab::Charts,
    ];

    pub fn title(self) -> &'static str {
        match self {
            Tab::Income           => "Income",
            Tab::Loans            => "Loans",
            Tab::PersonalExpenses => "Personal",
            Tab::FamilyExpenses   => "Family",
            Tab::OtherItems       => "Other",
            Tab::Summary          => "Summary",
            Tab::Spending         => "Spending",
            Tab::Import           => "Import",
            Tab::Charts           => "Charts",
        }
    }

    pub fn index(self) -> usize {
        Tab::ALL.iter().position(|t| *t == self).unwrap_or(0)
    }

    pub fn next(self) -> Self {
        let i = (self.index() + 1) % Tab::ALL.len();
        Tab::ALL[i]
    }

    pub fn prev(self) -> Self {
        let i = (self.index() + Tab::ALL.len() - 1) % Tab::ALL.len();
        Tab::ALL[i]
    }
}

// ── Edit mode ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EditMode {
    /// Normal navigation — arrows move between rows/fields.
    Normal,
    /// Actively editing a cell; keystrokes go into the edit buffer.
    Editing,
}

// ── Which field is focused within a row ──────────────────────────────────────

/// For tables with two member columns (Personal expenses: A and B).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PersonalField {
    Label,
    AmountA,
    AmountB,
}

impl PersonalField {
    pub fn next(self) -> Self {
        match self {
            Self::Label   => Self::AmountA,
            Self::AmountA => Self::AmountB,
            Self::AmountB => Self::Label,
        }
    }
    pub fn prev(self) -> Self {
        match self {
            Self::Label   => Self::AmountB,
            Self::AmountA => Self::Label,
            Self::AmountB => Self::AmountA,
        }
    }
}

/// Family expense fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FamilyField {
    Label,
    Total,
    AmountA,
    AmountB,
}

impl FamilyField {
    pub fn next(self) -> Self {
        match self {
            Self::Label   => Self::Total,
            Self::Total   => Self::AmountA,
            Self::AmountA => Self::AmountB,
            Self::AmountB => Self::Label,
        }
    }
    pub fn prev(self) -> Self {
        match self {
            Self::Label   => Self::AmountB,
            Self::Total   => Self::Label,
            Self::AmountA => Self::Total,
            Self::AmountB => Self::AmountA,
        }
    }
}

/// Income fields per member row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IncomeField {
    Name,
    AfterTax,
    PlEarly,
    PlLate,
}

impl IncomeField {
    pub fn next(self) -> Self {
        match self {
            Self::Name     => Self::AfterTax,
            Self::AfterTax => Self::PlEarly,
            Self::PlEarly  => Self::PlLate,
            Self::PlLate   => Self::Name,
        }
    }
    pub fn prev(self) -> Self {
        match self {
            Self::Name     => Self::PlLate,
            Self::AfterTax => Self::Name,
            Self::PlEarly  => Self::AfterTax,
            Self::PlLate   => Self::PlEarly,
        }
    }
}

/// Which subsection of the Loans tab is active.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoanSection {
    Mortgage,
    Car,
    Debts,
}

impl LoanSection {
    pub fn next(self) -> Self {
        match self {
            Self::Mortgage => Self::Car,
            Self::Car      => Self::Debts,
            Self::Debts    => Self::Mortgage,
        }
    }
    pub fn prev(self) -> Self {
        match self {
            Self::Mortgage => Self::Debts,
            Self::Car      => Self::Mortgage,
            Self::Debts    => Self::Car,
        }
    }
}

/// Editable input fields inside the Mortgage subsection.
/// MonthlyPrincipal, MonthlyInterest, and MonthlyTotal are computed — not editable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MortgageField {
    Principal,
    InterestRate,
    RemainingMonths,
    MonthlyInsurance,
    ShareA,
    Amortization,
}

impl MortgageField {
    pub const ALL: &'static [MortgageField] = &[
        Self::Principal,
        Self::InterestRate,
        Self::RemainingMonths,
        Self::MonthlyInsurance,
        Self::ShareA,
        Self::Amortization,
    ];
    pub fn next(self) -> Self {
        let i = Self::ALL.iter().position(|f| *f == self).unwrap_or(0);
        Self::ALL[(i + 1) % Self::ALL.len()]
    }
    pub fn prev(self) -> Self {
        let i = Self::ALL.iter().position(|f| *f == self).unwrap_or(0);
        Self::ALL[(i + Self::ALL.len() - 1) % Self::ALL.len()]
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Principal        => "Outstanding principal (残高)",
            Self::InterestRate     => "Annual interest rate % (年利)",
            Self::RemainingMonths  => "Remaining months (残期間)",
            Self::MonthlyInsurance => "Monthly insurance / fee (保証料等)",
            Self::ShareA           => "Member A share %",
            Self::Amortization     => "Amortization method (返済方式)",
        }
    }
}

/// Editable input fields inside the Car loan subsection.
/// MonthlyPrincipal, MonthlyInterest, and MonthlyTotal are computed — not editable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CarField {
    Principal,
    InterestRate,
    RemainingMonths,
    ShareA,
    Amortization,
}

impl CarField {
    pub const ALL: &'static [CarField] = &[
        Self::Principal,
        Self::InterestRate,
        Self::RemainingMonths,
        Self::ShareA,
        Self::Amortization,
    ];
    pub fn next(self) -> Self {
        let i = Self::ALL.iter().position(|f| *f == self).unwrap_or(0);
        Self::ALL[(i + 1) % Self::ALL.len()]
    }
    pub fn prev(self) -> Self {
        let i = Self::ALL.iter().position(|f| *f == self).unwrap_or(0);
        Self::ALL[(i + Self::ALL.len() - 1) % Self::ALL.len()]
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Principal       => "Outstanding principal (残高)",
            Self::InterestRate    => "Annual interest rate % (年利)",
            Self::RemainingMonths => "Remaining months (残期間)",
            Self::ShareA          => "Member A share %",
            Self::Amortization    => "Amortization method (返済方式)",
        }
    }
}

/// Fields inside a single Debt row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DebtField {
    Label,
    Principal,
    MonthlyPayment,
    InterestRate,
    RemainingMonths,
    ShareA,
}

impl DebtField {
    pub const ALL: &'static [DebtField] = &[
        Self::Label,
        Self::Principal,
        Self::MonthlyPayment,
        Self::InterestRate,
        Self::RemainingMonths,
        Self::ShareA,
    ];
    pub fn next(self) -> Self {
        let i = Self::ALL.iter().position(|f| *f == self).unwrap_or(0);
        Self::ALL[(i + 1) % Self::ALL.len()]
    }
    pub fn prev(self) -> Self {
        let i = Self::ALL.iter().position(|f| *f == self).unwrap_or(0);
        Self::ALL[(i + Self::ALL.len() - 1) % Self::ALL.len()]
    }
}

/// Other-items fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OtherField {
    Label,
    AnnualAmount,
    Notes,
}

impl OtherField {
    pub fn next(self) -> Self {
        match self {
            Self::Label        => Self::AnnualAmount,
            Self::AnnualAmount => Self::Notes,
            Self::Notes        => Self::Label,
        }
    }
    pub fn prev(self) -> Self {
        match self {
            Self::Label        => Self::Notes,
            Self::AnnualAmount => Self::Label,
            Self::Notes        => Self::AnnualAmount,
        }
    }
}

// ── Popups ────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Popup {
    None,
    MonthPicker,
    NewMonth,
    DeleteConfirm,
    Help,
    /// Category picker for a specific import row index.
    CategoryPicker { row: usize },
    /// Loan share member picker — which section and debt row triggered it.
    LoanMemberPicker,
    /// Import member picker — assigns a member to all imported transactions.
    ImportMemberPicker,
    /// Per-transaction member picker.
    /// row  = index into import_preview (is_spending=false) or drilled tx list (is_spending=true).
    MemberPicker { row: usize, is_spending: bool },
}

// ── Import state ──────────────────────────────────────────────────────────────

/// Which sub-field is focused in the Import tab.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportFocus {
    /// The file-path input field.
    FilePath,
    /// The provider selector.
    Provider,
    /// The parsed transaction list.
    TransactionList,
}

// ── Full App State ────────────────────────────────────────────────────────────

#[derive(Clone, PartialEq)]
pub struct App {
    // ── Data ─────────────────────────────────────────────────────────────────
    pub budget: Budget,
    /// All months stored in the DB (sorted).
    pub all_months: Vec<String>,
    /// Which month is currently loaded.
    pub current_month: String,
    /// Whether the current budget has unsaved changes.
    pub dirty: bool,

    // ── Scenario ─────────────────────────────────────────────────────────────
    pub scenario: IncomeScenario,

    // ── Navigation ───────────────────────────────────────────────────────────
    pub active_tab: Tab,
    pub edit_mode: EditMode,

    /// Selected row index within the active tab's table.
    pub selected_row: usize,

    /// Active column / field within the selected row.
    pub income_field:    IncomeField,
    pub loan_section:    LoanSection,
    pub mortgage_field:  MortgageField,
    pub car_field:       CarField,
    pub debt_field:      DebtField,
    pub personal_field:  PersonalField,
    pub family_field:    FamilyField,
    pub other_field:     OtherField,

    // ── Edit buffer ───────────────────────────────────────────────────────────
    /// Text currently being edited.
    pub edit_buf: String,
    /// Cursor position within edit_buf (byte index).
    pub edit_cursor: usize,

    // ── Popup ─────────────────────────────────────────────────────────────────
    pub popup: Popup,
    /// Row cursor inside the month-picker popup.
    pub popup_row: usize,
    /// Buffer for the "new month" input (YYYY-MM).
    pub new_month_buf: String,

    // ── Status bar ───────────────────────────────────────────────────────────
    pub status_msg: String,
    /// Ticks remaining to show the status message (decremented each frame).
    pub status_ttl: u8,

    // ── Import tab state ──────────────────────────────────────────────────────
    /// Which element is focused in the Import tab.
    pub import_focus: ImportFocus,
    /// File path the user is typing.
    pub import_path_buf: String,
    /// Cursor inside import_path_buf.
    pub import_path_cursor: usize,
    /// Currently selected card provider.
    pub import_provider: CardProvider,
    /// Parsed (but not yet committed) transactions from the last import parse.
    pub import_preview: Vec<Transaction>,
    /// Selected row inside the preview list.
    pub import_selected: usize,
    /// Cursor inside the category-picker list popup.
    pub import_cat_cursor: usize,
    /// Which member owns the currently-being-imported transactions ("" = Both/Shared, or member name)
    pub import_member: String,
    /// Cursor in the member-picker popup for import.
    pub import_member_cursor: usize,

    // ── Per-transaction member picker state ──────────────────────────────────
    /// Cursor inside the per-transaction member picker popup.
    pub spending_member_cursor: usize,

    // ── Loan share picker state ───────────────────────────────────────────────
    /// Cursor in the member-picker popup (0 = member A, 1 = member B).
    pub loan_share_picker_cursor: usize,
    /// Which member index (0 = A, 1 = B) was chosen when entering share edit.
    pub loan_share_editing_member: usize,

    // ── Spending tab state ────────────────────────────────────────────────────
    /// Selected row inside the spending categories list.
    pub spending_selected: usize,
    /// If true, show the individual transactions for the selected category.
    pub spending_drill: bool,
    /// Selected row inside the drilled-down transaction list.
    pub spending_tx_selected: usize,
}

impl App {
    pub fn new(budget: Budget, all_months: Vec<String>) -> Self {
        let current_month = budget.month.clone();
        Self {
            budget,
            all_months,
            current_month,
            dirty: false,
            scenario: IncomeScenario::Normal,
            active_tab: Tab::Income,
            edit_mode: EditMode::Normal,
            selected_row: 0,
            income_field: IncomeField::Name,
            loan_section: LoanSection::Mortgage,
            mortgage_field: MortgageField::Principal,
            car_field: CarField::Principal,
            debt_field: DebtField::Label,
            personal_field: PersonalField::Label,
            family_field: FamilyField::Label,
            other_field: OtherField::Label,
            edit_buf: String::new(),
            edit_cursor: 0,
            popup: Popup::None,
            popup_row: 0,
            new_month_buf: String::new(),
            status_msg: String::new(),
            status_ttl: 0,
            import_focus: ImportFocus::FilePath,
            import_path_buf: String::new(),
            import_path_cursor: 0,
            import_provider: CardProvider::RakutenCard,
            import_preview: Vec::new(),
            import_selected: 0,
            import_cat_cursor: 0,
            import_member: String::new(),
            import_member_cursor: 0,
            spending_member_cursor: 0,
            loan_share_picker_cursor: 0,
            loan_share_editing_member: 0,
            spending_selected: 0,
            spending_drill: false,
            spending_tx_selected: 0,
        }
    }

    // ── Status helper ─────────────────────────────────────────────────────────

    pub fn set_status(&mut self, msg: impl Into<String>) {
        self.status_msg = msg.into();
        self.status_ttl = 60; // ~3 s at 20 fps
    }

    pub fn tick_status(&mut self) {
        if self.status_ttl > 0 {
            self.status_ttl -= 1;
            if self.status_ttl == 0 {
                self.status_msg.clear();
            }
        }
    }

    // ── Row count helpers ─────────────────────────────────────────────────────

    pub fn income_row_count(&self) -> usize {
        self.budget.income.members.len()
    }

    pub fn loan_row_count(&self) -> usize {
        self.budget.loans.debts.len()
    }

    pub fn personal_row_count(&self) -> usize {
        self.budget.personal_expenses.items.len()
    }

    pub fn family_row_count(&self) -> usize {
        self.budget.family_expenses.items.len()
    }

    pub fn other_row_count(&self) -> usize {
        self.budget.other_items.items.len()
    }

    fn active_row_count(&self) -> usize {
        match self.active_tab {
            Tab::Income           => self.income_row_count(),
            Tab::Loans            => match self.loan_section {
                LoanSection::Mortgage => MortgageField::ALL.len(),
                LoanSection::Car      => CarField::ALL.len(),
                LoanSection::Debts    => self.budget.loans.debts.len(),
            },
            Tab::PersonalExpenses => self.personal_row_count(),
            Tab::FamilyExpenses   => self.family_row_count(),
            Tab::OtherItems       => self.other_row_count(),
            Tab::Summary          => 0,
            Tab::Spending         => {
                if self.spending_drill {
                    self.spending_drilled_tx_count()
                } else {
                    self.budget.spending.active_categories().len()
                }
            }
            Tab::Import           => self.import_preview.len(),
            Tab::Charts           => 0,
        }
    }

    // ── Navigation ───────────────────────────────────────────────────────────

    pub fn nav_tab_next(&mut self) {
        self.active_tab = self.active_tab.next();
        self.selected_row = 0;
    }

    pub fn nav_tab_prev(&mut self) {
        self.active_tab = self.active_tab.prev();
        self.selected_row = 0;
    }

    pub fn nav_row_down(&mut self) {
        let n = self.active_row_count();
        if n == 0 { return; }
        match self.active_tab {
            Tab::Loans => {
                match self.loan_section {
                    LoanSection::Mortgage => {
                        let i = MortgageField::ALL.iter()
                            .position(|f| *f == self.mortgage_field)
                            .unwrap_or(0);
                        let next = (i + 1).min(MortgageField::ALL.len() - 1);
                        self.mortgage_field = MortgageField::ALL[next];
                    }
                    LoanSection::Car => {
                        let i = CarField::ALL.iter()
                            .position(|f| *f == self.car_field)
                            .unwrap_or(0);
                        let next = (i + 1).min(CarField::ALL.len() - 1);
                        self.car_field = CarField::ALL[next];
                    }
                    LoanSection::Debts => {
                        self.selected_row = (self.selected_row + 1).min(n - 1);
                    }
                }
            }
            Tab::Spending => {
                if self.spending_drill {
                    self.spending_tx_selected = (self.spending_tx_selected + 1).min(n - 1);
                } else {
                    self.spending_selected = (self.spending_selected + 1).min(n - 1);
                }
            }
            Tab::Import => {
                self.import_selected = (self.import_selected + 1).min(n - 1);
            }
            _ => {
                self.selected_row = (self.selected_row + 1).min(n - 1);
            }
        }
    }

    pub fn nav_row_up(&mut self) {
        match self.active_tab {
            Tab::Loans => {
                match self.loan_section {
                    LoanSection::Mortgage => {
                        let i = MortgageField::ALL.iter()
                            .position(|f| *f == self.mortgage_field)
                            .unwrap_or(0);
                        if i > 0 { self.mortgage_field = MortgageField::ALL[i - 1]; }
                    }
                    LoanSection::Car => {
                        let i = CarField::ALL.iter()
                            .position(|f| *f == self.car_field)
                            .unwrap_or(0);
                        if i > 0 { self.car_field = CarField::ALL[i - 1]; }
                    }
                    LoanSection::Debts => {
                        if self.selected_row > 0 { self.selected_row -= 1; }
                    }
                }
            }
            Tab::Spending => {
                if self.spending_drill {
                    if self.spending_tx_selected > 0 { self.spending_tx_selected -= 1; }
                } else {
                    if self.spending_selected > 0 { self.spending_selected -= 1; }
                }
            }
            Tab::Import => {
                if self.import_selected > 0 { self.import_selected -= 1; }
            }
            _ => {
                if self.selected_row > 0 { self.selected_row -= 1; }
            }
        }
    }

    pub fn nav_col_next(&mut self) {
        match self.active_tab {
            Tab::Income           => self.income_field  = self.income_field.next(),
            Tab::Loans            => {
                match self.loan_section {
                    // In Debts: ← → moves between columns of the selected debt row.
                    LoanSection::Debts => {
                        let i = DebtField::ALL.iter()
                            .position(|f| *f == self.debt_field)
                            .unwrap_or(0);
                        let next = (i + 1).min(DebtField::ALL.len() - 1);
                        self.debt_field = DebtField::ALL[next];
                    }
                    // In Mortgage / Car: ← → switches to the next section panel.
                    _ => self.loan_section = self.loan_section.next(),
                }
            }
            Tab::PersonalExpenses => self.personal_field = self.personal_field.next(),
            Tab::FamilyExpenses   => self.family_field   = self.family_field.next(),
            Tab::OtherItems       => self.other_field    = self.other_field.next(),
            Tab::Import => {
                self.import_focus = match self.import_focus {
                    ImportFocus::FilePath       => ImportFocus::Provider,
                    ImportFocus::Provider       => ImportFocus::TransactionList,
                    ImportFocus::TransactionList => ImportFocus::FilePath,
                };
            }
            _ => {}
        }
    }

    pub fn nav_col_prev(&mut self) {
        match self.active_tab {
            Tab::Income           => self.income_field  = self.income_field.prev(),
            Tab::Loans            => {
                match self.loan_section {
                    // In Debts: ← → moves between columns of the selected debt row.
                    LoanSection::Debts => {
                        let i = DebtField::ALL.iter()
                            .position(|f| *f == self.debt_field)
                            .unwrap_or(0);
                        if i > 0 { self.debt_field = DebtField::ALL[i - 1]; }
                    }
                    // In Mortgage / Car: ← → switches to the previous section panel.
                    _ => self.loan_section = self.loan_section.prev(),
                }
            }
            Tab::PersonalExpenses => self.personal_field = self.personal_field.prev(),
            Tab::FamilyExpenses   => self.family_field   = self.family_field.prev(),
            Tab::OtherItems       => self.other_field    = self.other_field.prev(),
            Tab::Import => {
                self.import_focus = match self.import_focus {
                    ImportFocus::FilePath       => ImportFocus::TransactionList,
                    ImportFocus::Provider       => ImportFocus::FilePath,
                    ImportFocus::TransactionList => ImportFocus::Provider,
                };
            }
            _ => {}
        }
    }

    // ── Edit start / commit / cancel ─────────────────────────────────────────

    /// Read the current cell value into the edit buffer and enter Editing mode.
    pub fn begin_edit(&mut self) {
        // Amortization fields cycle on Enter instead of opening a text editor.
        // ShareA fields open the member-picker popup instead of a text editor.
        if self.active_tab == Tab::Loans {
            match self.loan_section {
                LoanSection::Mortgage if self.mortgage_field == MortgageField::Amortization => {
                    self.budget.loans.mortgage.amortization =
                        self.budget.loans.mortgage.amortization.cycle();
                    self.budget.loans.mortgage.recalculate();
                    self.dirty = true;
                    return;
                }
                LoanSection::Car if self.car_field == CarField::Amortization => {
                    self.budget.loans.car.amortization =
                        self.budget.loans.car.amortization.cycle();
                    self.budget.loans.car.recalculate();
                    self.dirty = true;
                    return;
                }
                LoanSection::Mortgage if self.mortgage_field == MortgageField::ShareA => {
                    self.loan_share_picker_cursor = 0;
                    self.popup = Popup::LoanMemberPicker;
                    return;
                }
                LoanSection::Car if self.car_field == CarField::ShareA => {
                    self.loan_share_picker_cursor = 0;
                    self.popup = Popup::LoanMemberPicker;
                    return;
                }
                LoanSection::Debts if self.debt_field == DebtField::ShareA => {
                    self.loan_share_picker_cursor = 0;
                    self.popup = Popup::LoanMemberPicker;
                    return;
                }
                _ => {}
            }
        }

        match self.active_tab {
            Tab::Import => {
                match self.import_focus {
                    ImportFocus::FilePath => {
                        self.edit_buf = self.import_path_buf.clone();
                        self.edit_cursor = self.edit_buf.len();
                        self.edit_mode = EditMode::Editing;
                    }
                    ImportFocus::TransactionList => {
                        // Open category picker for the selected import row.
                        if !self.import_preview.is_empty() {
                            self.import_cat_cursor = 0;
                            self.popup = Popup::CategoryPicker { row: self.import_selected };
                        }
                    }
                    ImportFocus::Provider => {
                        // Cycle provider on Enter.
                        self.cycle_import_provider();
                    }
                }
                return;
            }
            Tab::Spending => {
                if !self.spending_drill {
                    // Enter/e drills into the category.
                    self.spending_drill = true;
                    self.spending_tx_selected = 0;
                }
                return;
            }
            _ => {}
        }
        self.edit_buf = self.current_cell_value();
        self.edit_cursor = self.edit_buf.len();
        self.edit_mode = EditMode::Editing;
    }

    /// Called when the user confirms a member in the LoanMemberPicker popup.
    /// Records which member (0=A, 1=B) they chose and opens the numeric editor
    /// pre-filled with that member's current share percentage.
    pub fn confirm_loan_share_member(&mut self) {
        let member = self.loan_share_picker_cursor; // 0 = A, 1 = B
        self.loan_share_editing_member = member;
        self.popup = Popup::None;

        // Pre-fill the edit buffer with the chosen member's current share %.
        let share_a = match self.loan_section {
            LoanSection::Mortgage => self.budget.loans.mortgage.share_a,
            LoanSection::Car      => self.budget.loans.car.share_a,
            LoanSection::Debts    => self.budget.loans.debts
                .get(self.selected_row)
                .map(|d| d.share_a)
                .unwrap_or(0.5),
        };
        let pct = if member == 0 { share_a } else { 1.0 - share_a };
        self.edit_buf    = format!("{:.1}", pct * 100.0);
        self.edit_cursor = self.edit_buf.len();
        self.edit_mode   = EditMode::Editing;
    }

    /// Write the edit buffer back into the model. Returns an error string on
    /// parse failure (the edit is kept open so the user can correct it).
    pub fn commit_edit(&mut self) -> Result<(), String> {
        if self.active_tab == Tab::Import && self.import_focus == ImportFocus::FilePath {
            self.import_path_buf = self.edit_buf.trim().to_string();
            self.import_path_cursor = self.import_path_buf.len();
            self.edit_mode = EditMode::Normal;
            return Ok(());
        }

        let buf = self.edit_buf.trim().to_string();
        self.apply_cell_value(&buf)?;
        self.edit_mode = EditMode::Normal;
        self.dirty = true;
        Ok(())
    }

    pub fn cancel_edit(&mut self) {
        self.edit_buf.clear();
        self.edit_cursor = 0;
        self.edit_mode = EditMode::Normal;
    }

    // ── Edit buffer manipulation ──────────────────────────────────────────────

    pub fn edit_insert_char(&mut self, c: char) {
        self.edit_buf.insert(self.edit_cursor, c);
        self.edit_cursor += c.len_utf8();
    }

    pub fn edit_backspace(&mut self) {
        if self.edit_cursor == 0 { return; }
        let mut i = self.edit_cursor - 1;
        while !self.edit_buf.is_char_boundary(i) { i -= 1; }
        self.edit_buf.remove(i);
        self.edit_cursor = i;
    }

    pub fn edit_delete(&mut self) {
        if self.edit_cursor >= self.edit_buf.len() { return; }
        self.edit_buf.remove(self.edit_cursor);
    }

    pub fn edit_cursor_left(&mut self) {
        if self.edit_cursor == 0 { return; }
        let mut i = self.edit_cursor - 1;
        while !self.edit_buf.is_char_boundary(i) { i -= 1; }
        self.edit_cursor = i;
    }

    pub fn edit_cursor_right(&mut self) {
        if self.edit_cursor >= self.edit_buf.len() { return; }
        let mut i = self.edit_cursor + 1;
        while i < self.edit_buf.len() && !self.edit_buf.is_char_boundary(i) { i += 1; }
        self.edit_cursor = i;
    }

    pub fn edit_cursor_home(&mut self) { self.edit_cursor = 0; }
    pub fn edit_cursor_end(&mut self)  { self.edit_cursor = self.edit_buf.len(); }

    // ── Row add / delete ─────────────────────────────────────────────────────

    pub fn add_row(&mut self) {
        match self.active_tab {
            Tab::Loans => {
                if self.loan_section == LoanSection::Debts {
                    self.budget.loans.debts.push(Debt::new("New debt"));
                    self.selected_row = self.budget.loans.debts.len() - 1;
                    self.dirty = true;
                }
            }
            Tab::PersonalExpenses => {
                self.budget.personal_expenses.items.push(PersonalExpenseItem {
                    label: "New item".into(),
                    amount_a: 0,
                    amount_b: 0,
                });
                self.selected_row = self.budget.personal_expenses.items.len() - 1;
                self.dirty = true;
            }
            Tab::FamilyExpenses => {
                self.budget.family_expenses.items.push(FamilyExpenseItem {
                    label: "New item".into(),
                    total: 0,
                    amount_a: 0,
                    amount_b: 0,
                });
                self.selected_row = self.budget.family_expenses.items.len() - 1;
                self.dirty = true;
            }
            Tab::OtherItems => {
                self.budget.other_items.items.push(OtherItem {
                    label: "New item".into(),
                    annual_amount: 0,
                    notes: String::new(),
                });
                self.selected_row = self.budget.other_items.items.len() - 1;
                self.dirty = true;
            }
            _ => {}
        }
    }

    pub fn delete_row(&mut self) {
        let r = self.selected_row;
        match self.active_tab {
            Tab::PersonalExpenses => {
                if r < self.budget.personal_expenses.items.len() {
                    self.budget.personal_expenses.items.remove(r);
                    self.dirty = true;
                }
            }
            Tab::FamilyExpenses => {
                if r < self.budget.family_expenses.items.len() {
                    self.budget.family_expenses.items.remove(r);
                    self.dirty = true;
                }
            }
            Tab::OtherItems => {
                if r < self.budget.other_items.items.len() {
                    self.budget.other_items.items.remove(r);
                    self.dirty = true;
                }
            }
            Tab::Loans => {
                // Only Debts section supports adding/removing rows.
                if self.loan_section == LoanSection::Debts {
                    if r < self.budget.loans.debts.len() {
                        self.budget.loans.debts.remove(r);
                        self.dirty = true;
                        let n = self.budget.loans.debts.len();
                        if n > 0 && self.selected_row >= n {
                            self.selected_row = n - 1;
                        }
                    }
                }
            }
            Tab::Import => {
                // Delete from preview (discard this transaction).
                if r < self.import_preview.len() {
                    self.import_preview.remove(r);
                    if !self.import_preview.is_empty() && self.import_selected >= self.import_preview.len() {
                        self.import_selected = self.import_preview.len() - 1;
                    }
                }
            }
            Tab::Spending => {
                // Delete a transaction from the drilled view.
                if self.spending_drill {
                    let cat_name = self.spending_drilled_category_name();
                    if let Some(cat_name) = cat_name {
                        let indices: Vec<usize> = self.budget.spending.transactions
                            .iter()
                            .enumerate()
                            .filter(|(_, tx)| tx.category == cat_name)
                            .map(|(i, _)| i)
                            .collect();
                        if let Some(&idx) = indices.get(self.spending_tx_selected) {
                            self.budget.spending.transactions.remove(idx);
                            self.dirty = true;
                            let new_len = self.spending_drilled_tx_count();
                            if new_len == 0 {
                                self.spending_drill = false;
                            } else if self.spending_tx_selected >= new_len {
                                self.spending_tx_selected = new_len - 1;
                            }
                        }
                    }
                }
            }
            _ => {}
        }
        // Clamp cursor for standard tabs.
        if !matches!(self.active_tab, Tab::Spending | Tab::Import) {
            let n = self.active_row_count();
            if n > 0 && self.selected_row >= n {
                self.selected_row = n - 1;
            }
        }
    }

    // ── Scenario cycling ──────────────────────────────────────────────────────

    pub fn cycle_scenario(&mut self) {
        self.scenario = match self.scenario {
            IncomeScenario::Normal             => IncomeScenario::ParentalLeaveEarly,
            IncomeScenario::ParentalLeaveEarly => IncomeScenario::ParentalLeaveLate,
            IncomeScenario::ParentalLeaveLate  => IncomeScenario::Normal,
        };
    }

    pub fn scenario_label(&self) -> &'static str {
        match self.scenario {
            IncomeScenario::Normal             => "Normal",
            IncomeScenario::ParentalLeaveEarly => "Parental Leave (~180d)",
            IncomeScenario::ParentalLeaveLate  => "Parental Leave (180d+)",
        }
    }

    // ── Import helpers ────────────────────────────────────────────────────────

    /// Cycle to the next available card provider.
    pub fn cycle_import_provider(&mut self) {
        let all = CardProvider::ALL;
        let idx = all.iter().position(|p| *p == self.import_provider).unwrap_or(0);
        self.import_provider = all[(idx + 1) % all.len()];
    }

    /// Assign a category to the currently selected import preview transaction.
    pub fn import_assign_category(&mut self, category: String) {
        if let Some(tx) = self.import_preview.get_mut(self.import_selected) {
            tx.category = category;
        }
        // Advance to the next uncategorized row.
        let next = self.import_preview
            .iter()
            .enumerate()
            .skip(self.import_selected + 1)
            .find(|(_, tx)| tx.category.is_empty())
            .map(|(i, _)| i);
        if let Some(i) = next {
            self.import_selected = i;
        }
        self.popup = Popup::None;
    }

    /// Assign a category to a specific import preview row by index.
    pub fn import_assign_category_for_row(&mut self, row: usize, category: String) {
        if let Some(tx) = self.import_preview.get_mut(row) {
            tx.category = category;
        }
        // Advance import_selected to next uncategorized.
        let next = self.import_preview
            .iter()
            .enumerate()
            .find(|(i, tx)| *i > row && tx.category.is_empty())
            .map(|(i, _)| i);
        if let Some(i) = next {
            self.import_selected = i;
        }
        self.popup = Popup::None;
    }

    /// Commit all previewed (categorized) transactions into the budget spending log.
    /// Uncategorized transactions get placed in "Uncategorized".
    pub fn import_commit(&mut self) {
        let member = self.import_member.clone();
        for mut tx in self.import_preview.drain(..) {
            if tx.category.is_empty() {
                tx.category = "Uncategorized".to_string();
            }
            if !member.is_empty() {
                tx.member = member.clone();
            }
            self.budget.spending.transactions.push(tx);
        }
        self.budget.sync_spending_categories();
        self.import_selected = 0;
        self.dirty = true;
    }

    /// Return the display label for the current import member selection.
    pub fn import_member_label(&self) -> String {
        if self.import_member.is_empty() {
            "Both / Shared".to_string()
        } else {
            self.import_member.clone()
        }
    }

    /// Open the import member picker popup.
    pub fn open_import_member_picker(&mut self) {
        let names: Vec<String> = {
            let mut v = vec!["Both / Shared".to_string()];
            v.extend(self.budget.income.members.iter().map(|m| m.name.clone()));
            v
        };
        let cur = names.iter().position(|n| {
            if self.import_member.is_empty() { n == "Both / Shared" }
            else { *n == self.import_member }
        }).unwrap_or(0);
        self.import_member_cursor = cur;
        self.popup = Popup::ImportMemberPicker;
    }

    /// Confirm the selection in the import member picker popup.
    pub fn confirm_import_member(&mut self) {
        let names: Vec<String> = {
            let mut v = vec!["Both / Shared".to_string()];
            v.extend(self.budget.income.members.iter().map(|m| m.name.clone()));
            v
        };
        if let Some(name) = names.get(self.import_member_cursor) {
            self.import_member = if name == "Both / Shared" {
                String::new()
            } else {
                name.clone()
            };
        }
        self.popup = Popup::None;
    }

    /// Open the per-transaction member picker for an import preview row or a
    /// drilled spending transaction.
    pub fn open_tx_member_picker(&mut self, row: usize, is_spending: bool) {
        let names = self.member_picker_names();
        let cur_member = if is_spending {
            let cat = self.spending_drilled_category_name();
            if let Some(cat) = cat {
                self.budget.spending.transactions
                    .iter()
                    .filter(|t| t.category == cat)
                    .nth(row)
                    .map(|t| t.member.clone())
                    .unwrap_or_default()
            } else {
                String::new()
            }
        } else {
            self.import_preview.get(row).map(|t| t.member.clone()).unwrap_or_default()
        };
        self.spending_member_cursor = names.iter()
            .position(|n| {
                if cur_member.is_empty() { n == "Both / Shared" }
                else { *n == cur_member }
            })
            .unwrap_or(0);
        self.popup = Popup::MemberPicker { row, is_spending };
    }

    /// Confirm a per-transaction member picker selection.
    pub fn confirm_tx_member(&mut self, row: usize, is_spending: bool) {
        let names = self.member_picker_names();
        let chosen = names.get(self.spending_member_cursor).cloned().unwrap_or_default();
        let member = if chosen == "Both / Shared" { String::new() } else { chosen };

        if is_spending {
            let cat = self.spending_drilled_category_name();
            if let Some(cat) = cat {
                let indices: Vec<usize> = self.budget.spending.transactions
                    .iter()
                    .enumerate()
                    .filter(|(_, t)| t.category == cat)
                    .map(|(i, _)| i)
                    .collect();
                if let Some(&idx) = indices.get(row) {
                    self.budget.spending.transactions[idx].member = member;
                    self.dirty = true;
                }
            }
        } else if let Some(tx) = self.import_preview.get_mut(row) {
            tx.member = member;
        }
        self.popup = Popup::None;
    }

    /// Helper — ordered list of choices for any member picker popup.
    pub fn member_picker_names(&self) -> Vec<String> {
        let mut v = vec!["Both / Shared".to_string()];
        v.extend(self.budget.income.members.iter().map(|m| m.name.clone()));
        v
    }

    /// Clear the import preview without committing.
    pub fn import_clear_preview(&mut self) {
        self.import_preview.clear();
        self.import_selected = 0;
    }

    // ── Spending helpers ──────────────────────────────────────────────────────

    /// Return the category name for the currently selected spending row (if drilling).
    pub fn spending_drilled_category_name(&self) -> Option<String> {
        let cats = self.budget.spending.active_categories();
        cats.get(self.spending_selected).cloned()
    }

    /// Return transactions for the drilled-into category.
    pub fn spending_drilled_transactions(&self) -> Vec<&Transaction> {
        if let Some(cat_name) = self.spending_drilled_category_name() {
            self.budget.spending.transactions
                .iter()
                .filter(|tx| tx.category == cat_name)
                .collect()
        } else {
            Vec::new()
        }
    }

    /// Count transactions for the drilled category.
    fn spending_drilled_tx_count(&self) -> usize {
        self.spending_drilled_transactions().len()
    }

    /// Reassign the currently selected drilled transaction to a new category.
    pub fn spending_reassign_category(&mut self, new_category: String) {
        if let Some(cat_name) = self.spending_drilled_category_name() {
            let indices: Vec<usize> = self.budget.spending.transactions
                .iter()
                .enumerate()
                .filter(|(_, tx)| tx.category == cat_name)
                .map(|(i, _)| i)
                .collect();
            if let Some(&idx) = indices.get(self.spending_tx_selected) {
                self.budget.spending.transactions[idx].category = new_category;
                self.budget.sync_spending_categories();
                self.dirty = true;
                // If no more transactions in this category, exit drill view.
                if self.spending_drilled_tx_count() == 0 {
                    self.spending_drill = false;
                }
            }
        }
        self.popup = Popup::None;
    }

    // ── Cell value read / write ───────────────────────────────────────────────

    /// Return the string representation of the currently focused cell.
    fn current_cell_value(&self) -> String {
        let r = self.selected_row;
        match self.active_tab {
            Tab::Income => {
                if let Some(m) = self.budget.income.members.get(r) {
                    match self.income_field {
                        IncomeField::Name     => m.name.clone(),
                        IncomeField::AfterTax => m.income_after_tax.to_string(),
                        IncomeField::PlEarly  => m.parental_leave_early.to_string(),
                        IncomeField::PlLate   => m.parental_leave_late.to_string(),
                    }
                } else { String::new() }
            }
            Tab::Loans => {
                match self.loan_section {
                    LoanSection::Mortgage => {
                        let m = &self.budget.loans.mortgage;
                        match self.mortgage_field {
                            MortgageField::Principal        => m.principal.to_string(),
                            MortgageField::InterestRate     => format!("{:.4}", m.interest_rate * 100.0),
                            MortgageField::RemainingMonths  => m.remaining_months.to_string(),
                            MortgageField::MonthlyInsurance => m.monthly_insurance.to_string(),
                            MortgageField::ShareA           => {
                                let pct = if self.loan_share_editing_member == 0 { m.share_a } else { 1.0 - m.share_a };
                                format!("{:.1}", pct * 100.0)
                            }
                            // Amortization just shows the label; toggled with Enter, not free-typed.
                            MortgageField::Amortization     => m.amortization.label().to_string(),
                        }
                    }
                    LoanSection::Car => {
                        let c = &self.budget.loans.car;
                        match self.car_field {
                            CarField::Principal        => c.principal.to_string(),
                            CarField::InterestRate     => format!("{:.4}", c.interest_rate * 100.0),
                            CarField::RemainingMonths  => c.remaining_months.to_string(),
                            CarField::ShareA           => {
                                let pct = if self.loan_share_editing_member == 0 { c.share_a } else { 1.0 - c.share_a };
                                format!("{:.1}", pct * 100.0)
                            }
                            CarField::Amortization     => c.amortization.label().to_string(),
                        }
                    }
                    LoanSection::Debts => {
                        if let Some(d) = self.budget.loans.debts.get(r) {
                            match self.debt_field {
                                DebtField::Label           => d.label.clone(),
                                DebtField::Principal       => d.principal.to_string(),
                                DebtField::MonthlyPayment  => d.monthly_payment.to_string(),
                                DebtField::InterestRate    => format!("{:.4}", d.interest_rate * 100.0),
                                DebtField::RemainingMonths => d.remaining_months.to_string(),
                                DebtField::ShareA          => {
                                    let pct = if self.loan_share_editing_member == 0 { d.share_a } else { 1.0 - d.share_a };
                                    format!("{:.1}", pct * 100.0)
                                }
                            }
                        } else { String::new() }
                    }
                }
            }
            Tab::PersonalExpenses => {
                if let Some(item) = self.budget.personal_expenses.items.get(r) {
                    match self.personal_field {
                        PersonalField::Label   => item.label.clone(),
                        PersonalField::AmountA => item.amount_a.to_string(),
                        PersonalField::AmountB => item.amount_b.to_string(),
                    }
                } else { String::new() }
            }
            Tab::FamilyExpenses => {
                if let Some(item) = self.budget.family_expenses.items.get(r) {
                    match self.family_field {
                        FamilyField::Label   => item.label.clone(),
                        FamilyField::Total   => item.total.to_string(),
                        FamilyField::AmountA => item.amount_a.to_string(),
                        FamilyField::AmountB => item.amount_b.to_string(),
                    }
                } else { String::new() }
            }
            Tab::OtherItems => {
                if let Some(item) = self.budget.other_items.items.get(r) {
                    match self.other_field {
                        OtherField::Label        => item.label.clone(),
                        OtherField::AnnualAmount => item.annual_amount.to_string(),
                        OtherField::Notes        => item.notes.clone(),
                    }
                } else { String::new() }
            }
            Tab::Summary | Tab::Spending | Tab::Import | Tab::Charts => String::new(),
        }
    }

    /// Parse `buf` and write it into the focused cell. Returns `Err(msg)` on bad input.
    fn apply_cell_value(&mut self, buf: &str) -> Result<(), String> {
        let r = self.selected_row;
        match self.active_tab {
            Tab::Income => {
                if let Some(m) = self.budget.income.members.get_mut(r) {
                    match self.income_field {
                        IncomeField::Name     => m.name = buf.to_string(),
                        IncomeField::AfterTax => m.income_after_tax = parse_jpy(buf)?,
                        IncomeField::PlEarly  => m.parental_leave_early = parse_jpy(buf)?,
                        IncomeField::PlLate   => m.parental_leave_late = parse_jpy(buf)?,
                    }
                }
            }
            Tab::Loans => {
                match self.loan_section {
                    LoanSection::Mortgage => {
                        let m = &mut self.budget.loans.mortgage;
                        match self.mortgage_field {
                            MortgageField::Principal        => m.principal        = parse_jpy(buf)?,
                            MortgageField::InterestRate     => m.interest_rate    = parse_rate(buf)?,
                            MortgageField::RemainingMonths  => m.remaining_months = parse_u32(buf)?,
                            MortgageField::MonthlyInsurance => m.monthly_insurance = parse_jpy(buf)?,
                            MortgageField::ShareA           => {
                                let pct = parse_pct(buf)?.clamp(0.0, 1.0);
                                m.share_a = if self.loan_share_editing_member == 0 { pct } else { 1.0 - pct };
                            }
                            // Amortization is toggled via begin_edit, not free-typed — nothing to parse.
                            MortgageField::Amortization     => {}
                        }
                        self.budget.loans.mortgage.recalculate();
                    }
                    LoanSection::Car => {
                        let c = &mut self.budget.loans.car;
                        match self.car_field {
                            CarField::Principal        => c.principal         = parse_jpy(buf)?,
                            CarField::InterestRate     => c.interest_rate     = parse_rate(buf)?,
                            CarField::RemainingMonths  => c.remaining_months  = parse_u32(buf)?,
                            CarField::ShareA           => {
                                let pct = parse_pct(buf)?.clamp(0.0, 1.0);
                                c.share_a = if self.loan_share_editing_member == 0 { pct } else { 1.0 - pct };
                            }
                            CarField::Amortization     => {}
                        }
                        self.budget.loans.car.recalculate();
                    }
                    LoanSection::Debts => {
                        if let Some(d) = self.budget.loans.debts.get_mut(r) {
                            match self.debt_field {
                                DebtField::Label           => d.label           = buf.to_string(),
                                DebtField::Principal       => d.principal       = parse_jpy(buf)?,
                                DebtField::MonthlyPayment  => d.monthly_payment = parse_jpy(buf)?,
                                DebtField::InterestRate    => d.interest_rate   = parse_rate(buf)?,
                                DebtField::RemainingMonths => d.remaining_months = parse_u32(buf)?,
                                DebtField::ShareA          => {
                                    let pct = parse_pct(buf)?.clamp(0.0, 1.0);
                                    d.share_a = if self.loan_share_editing_member == 0 { pct } else { 1.0 - pct };
                                }
                            }
                        }
                    }
                }
            }
            Tab::PersonalExpenses => {
                if let Some(item) = self.budget.personal_expenses.items.get_mut(r) {
                    match self.personal_field {
                        PersonalField::Label   => item.label = buf.to_string(),
                        PersonalField::AmountA => item.amount_a = parse_jpy(buf)?,
                        PersonalField::AmountB => item.amount_b = parse_jpy(buf)?,
                    }
                }
            }
            Tab::FamilyExpenses => {
                if let Some(item) = self.budget.family_expenses.items.get_mut(r) {
                    match self.family_field {
                        FamilyField::Label => item.label = buf.to_string(),
                        FamilyField::Total => {
                            let v = parse_jpy(buf)?;
                            let ratio = if item.total != 0 {
                                item.amount_a as f64 / item.total as f64
                            } else {
                                0.4
                            };
                            item.total = v;
                            item.amount_a = (v as f64 * ratio).round() as i64;
                            item.amount_b = v - item.amount_a;
                        }
                        FamilyField::AmountA => {
                            item.amount_a = parse_jpy(buf)?;
                            item.amount_b = item.total - item.amount_a;
                        }
                        FamilyField::AmountB => {
                            item.amount_b = parse_jpy(buf)?;
                            item.amount_a = item.total - item.amount_b;
                        }
                    }
                }
            }
            Tab::OtherItems => {
                if let Some(item) = self.budget.other_items.items.get_mut(r) {
                    match self.other_field {
                        OtherField::Label        => item.label = buf.to_string(),
                        OtherField::AnnualAmount => item.annual_amount = parse_jpy(buf)?,
                        OtherField::Notes        => item.notes = buf.to_string(),
                    }
                }
            }
            Tab::Summary | Tab::Spending | Tab::Import | Tab::Charts => {}
        }
        Ok(())
    }
}

// ── Number parsers ────────────────────────────────────────────────────────────

pub fn parse_jpy(s: &str) -> Result<i64, String> {
    let clean: String = s.chars()
        .filter(|c| c.is_ascii_digit() || *c == '-')
        .collect();
    clean.parse::<i64>()
        .map_err(|_| format!("'{}' is not a valid JPY amount (whole numbers only)", s))
}

pub fn parse_pct(s: &str) -> Result<f64, String> {
    let clean: String = s.chars()
        .filter(|c| c.is_ascii_digit() || *c == '.' || *c == '-')
        .collect();
    let v: f64 = clean.parse()
        .map_err(|_| format!("'{}' is not a valid percentage", s))?;
    Ok(v / 100.0)
}

/// Parse an interest rate entered as e.g. "0.6" or "0.6%" → 0.006 (stored as fraction).
pub fn parse_rate(s: &str) -> Result<f64, String> {
    let clean: String = s.chars()
        .filter(|c| c.is_ascii_digit() || *c == '.' || *c == '-')
        .collect();
    let v: f64 = clean.parse()
        .map_err(|_| format!("'{}' is not a valid rate", s))?;
    // Accept either 0.6 (meaning 0.6%) or 0.006 (already a fraction).
    // Heuristic: if > 0.2 it's entered as percentage points.
    Ok(if v > 0.2 { v / 100.0 } else { v })
}

/// Parse a non-negative integer (months count, etc.).
pub fn parse_u32(s: &str) -> Result<u32, String> {
    let clean: String = s.chars().filter(|c| c.is_ascii_digit()).collect();
    clean.parse::<u32>()
        .map_err(|_| format!("'{}' is not a valid whole number", s))
}
