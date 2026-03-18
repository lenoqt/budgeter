//! Data model for the family budget application.
//! All monetary values are in JPY (integers for yen, no decimals needed).

use serde::{Deserialize, Serialize};

// ── Transactions / Spending ───────────────────────────────────────────────────

/// A single imported credit card transaction.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transaction {
    /// ISO date string "YYYY-MM-DD"
    pub date: String,
    /// Original merchant name from the card statement (full, verbatim)
    pub merchant: String,
    /// Card holder name as printed on statement (e.g. "本人")
    pub cardholder: String,
    /// Payment method description (e.g. "1回払い", "分割24回払い(11回目)")
    pub payment_method: String,
    /// Original purchase amount in JPY
    pub amount: i64,
    /// Fee / interest for installment payments
    pub fee: i64,
    /// Amount due THIS statement month
    pub amount_this_month: i64,
    /// Budget category chosen by the user. Empty = not yet categorized.
    pub category: String,
    /// Card provider that produced this row (e.g. "Rakuten Card")
    pub provider: String,
}

impl Transaction {
    pub fn is_installment(&self) -> bool {
        !self.payment_method.contains("1回払い")
    }
}

/// One budget category linked to spending tracking.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpendingCategory {
    pub name: String,
    /// Budgeted amount copied from the budget plan for this month.
    pub budgeted: i64,
}

impl SpendingCategory {
    pub fn new(name: impl Into<String>, budgeted: i64) -> Self {
        Self { name: name.into(), budgeted }
    }
}

/// All imported transactions + category caps for a given month.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SpendingLog {
    pub transactions: Vec<Transaction>,
    /// Category budget caps (built from the budget plan on import).
    pub categories: Vec<SpendingCategory>,
}

impl SpendingLog {
    pub fn total_this_month(&self) -> i64 {
        self.transactions.iter().map(|t| t.amount_this_month).sum()
    }

    pub fn total_for_category(&self, cat: &str) -> i64 {
        self.transactions
            .iter()
            .filter(|t| t.category.eq_ignore_ascii_case(cat))
            .map(|t| t.amount_this_month)
            .sum()
    }

    pub fn remaining_for_category(&self, cat: &str) -> Option<i64> {
        self.categories
            .iter()
            .find(|c| c.name.eq_ignore_ascii_case(cat))
            .map(|c| c.budgeted - self.total_for_category(cat))
    }

    /// Return all unique categories that appear in the transactions, sorted.
    pub fn active_categories(&self) -> Vec<String> {
        let mut v: Vec<String> = self.transactions.iter().map(|t| t.category.clone()).collect();
        v.sort();
        v.dedup();
        v
    }

    /// Total uncategorized spend.
    pub fn total_uncategorized(&self) -> i64 {
        self.transactions
            .iter()
            .filter(|t| t.category.is_empty() || t.category == "Uncategorized")
            .map(|t| t.amount_this_month)
            .sum()
    }
}

/// Known card providers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CardProvider {
    RakutenCard,
}

impl CardProvider {
    pub const ALL: &'static [CardProvider] = &[CardProvider::RakutenCard];

    pub fn label(self) -> &'static str {
        match self {
            CardProvider::RakutenCard => "Rakuten Card (楽天カード)",
        }
    }
}

// ── Income ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IncomeMember {
    pub name: String,
    pub income_after_tax: i64,
    pub parental_leave_early: i64,
    pub parental_leave_late: i64,
}

impl IncomeMember {
    pub fn new(name: &str, after_tax: i64, pl_early: i64, pl_late: i64) -> Self {
        Self {
            name: name.to_string(),
            income_after_tax: after_tax,
            parental_leave_early: pl_early,
            parental_leave_late: pl_late,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Income {
    pub members: Vec<IncomeMember>,
}

impl Income {
    pub fn total_after_tax(&self) -> i64 { self.members.iter().map(|m| m.income_after_tax).sum() }
    pub fn total_pl_early(&self) -> i64  { self.members.iter().map(|m| m.parental_leave_early).sum() }
    pub fn total_pl_late(&self) -> i64   { self.members.iter().map(|m| m.parental_leave_late).sum() }
}

// ── Loans ────────────────────────────────────────────────────────────────────

/// Amortization method for a loan.
///
/// - `FixedPayment`   (元利均等返済): the total monthly payment stays constant.
///   Interest is front-loaded; the principal portion grows over time.
///   Formula: payment = P × r / (1 − (1+r)^−n)  where r = monthly rate, n = months.
///
/// - `FixedPrincipal` (元金均等返済): the principal repayment is constant each month.
///   Interest decreases as the balance falls, so the total payment shrinks over time.
///   This month's payment = P/n + P×r  (using current outstanding principal P).
///
/// - `French` (French/Français): mathematically identical to FixedPayment (constant
///   annuity), but the label is used in many European/international contexts.
///   Included as a distinct variant for clarity when the loan contract uses this term.
///
/// - `German` (German/Deutsch): identical to FixedPrincipal (constant principal
///   repayment each period), the standard naming used in German-speaking markets and
///   many international loan agreements.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AmortizationMethod {
    /// 元利均等返済 — constant total payment (JP standard)
    FixedPayment,
    /// 元金均等返済 — constant principal portion (JP standard)
    FixedPrincipal,
    /// French method — constant annuity (same math as FixedPayment, European label)
    French,
    /// German method — constant principal (same math as FixedPrincipal, European label)
    German,
}

impl AmortizationMethod {
    pub const ALL: &'static [AmortizationMethod] = &[
        Self::FixedPayment,
        Self::FixedPrincipal,
        Self::French,
        Self::German,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::FixedPayment   => "Fixed payment   (元利均等)",
            Self::FixedPrincipal => "Fixed principal (元金均等)",
            Self::French         => "French method   (定額返済)",
            Self::German         => "German method   (定額元金)",
        }
    }

    /// Cycle to the next method in the list, wrapping around.
    pub fn cycle(self) -> Self {
        let i = Self::ALL.iter().position(|m| *m == self).unwrap_or(0);
        Self::ALL[(i + 1) % Self::ALL.len()]
    }

    /// Whether this method uses constant-principal math (FixedPrincipal / German).
    fn is_fixed_principal(self) -> bool {
        matches!(self, Self::FixedPrincipal | Self::German)
    }
}

impl Default for AmortizationMethod {
    fn default() -> Self { Self::FixedPayment }
}

// ── Amortization calculation helpers ─────────────────────────────────────────

/// Result of one month's amortization calculation.
///
/// `monthly_payment` is the mathematically exact annuity rounded to the
/// nearest yen.  `rounded_payment` rounds further to the nearest ¥100
/// (common Japanese dealer convention). When a dealer uses ¥100 rounding
/// they collect the shortfall as a bump on the very first payment, stored
/// in `first_payment`.
#[derive(Debug, Clone, Copy, Default)]
pub struct AmortResult {
    pub monthly_principal: i64,
    pub monthly_interest:  i64,
    /// Exact annuity payment rounded to nearest ¥1 (principal + interest).
    pub monthly_payment:   i64,
    /// Payment rounded down to nearest ¥100 — matches dealer quote style.
    pub rounded_payment:   i64,
    /// First payment when using ¥100 rounding: rounded_payment + shortfall.
    /// For a standard (non-rounded) loan this equals monthly_payment.
    pub first_payment:     i64,
}

/// Compute this month's principal and interest split for a given loan.
///
/// **`principal` must be the actual loan balance borrowed (元金残高) — NOT
/// the total installment price (割賦販売価格) which already includes all
/// accumulated interest charges.**
///
/// ### Why the dealer's figure can differ from the exact annuity
/// Japanese auto dealers commonly:
/// 1. Compute the exact annuity (e.g. ¥22,329).
/// 2. Round it **down** to the nearest ¥100 (→ ¥22,300).
/// 3. Collect the total rounding shortfall (29 × 60 = ¥1,740) as a bump
///    on the first payment (¥22,300 + ¥1,746 = ¥24,046, the extra ¥6
///    being stamp/handling fees).
/// The app shows both the exact payment and the ¥100-rounded version so
/// you can match either figure.
///
/// Returns all-zero `AmortResult` when inputs are degenerate (zero
/// principal, zero rate, or zero remaining months).
pub fn amort_calc(
    principal: i64,
    annual_rate: f64,
    remaining_months: u32,
    method: AmortizationMethod,
) -> AmortResult {
    if principal <= 0 || remaining_months == 0 {
        return AmortResult::default();
    }

    let n   = remaining_months as f64;
    let p   = principal as f64;
    let r   = annual_rate / 12.0; // monthly rate

    match method {
        // ── Fixed payment / French — constant annuity (元利均等返済) ──────────
        // Formula: M = P × r / (1 − (1+r)^−n)
        // We keep the payment as a float until the very last step so that the
        // interest + principal split always sums back to monthly_payment exactly.
        method if !method.is_fixed_principal() => {
            // Exact annuity payment (float, before rounding).
            let payment_f: f64 = if r < 1e-12 {
                p / n   // zero-rate: equal slices
            } else {
                let factor = r / (1.0 - (1.0 + r).powf(-n));
                p * factor
            };

            // Interest on the current balance (float, before rounding).
            let interest_f  = p * r;

            // Round each component independently; keep them consistent.
            let monthly_payment   = payment_f.round() as i64;
            let monthly_interest  = interest_f.round() as i64;
            let monthly_principal = (monthly_payment - monthly_interest).max(0);

            // Dealer-style ¥100 rounding: floor to nearest 100.
            let rounded_payment = (monthly_payment / 100) * 100;
            // Shortfall per month × n months = total shortfall added to first payment.
            let shortfall   = (monthly_payment - rounded_payment) * remaining_months as i64;
            let first_payment = rounded_payment + shortfall;

            AmortResult {
                monthly_principal,
                monthly_interest,
                monthly_payment,
                rounded_payment,
                first_payment,
            }
        }

        // ── Fixed principal / German — constant principal (元金均等返済) ──────
        // Principal slice is constant; interest falls each month as balance drops.
        _ => {
            let principal_part = (p / n).round() as i64;
            let interest_part  = (p * r).round() as i64;
            let monthly_payment = principal_part + interest_part;

            AmortResult {
                monthly_principal: principal_part,
                monthly_interest:  interest_part,
                monthly_payment,
                // Fixed-principal loans don't use dealer rounding — show exact.
                rounded_payment:   monthly_payment,
                first_payment:     monthly_payment,
            }
        }
    }
}

// ── Mortgage ──────────────────────────────────────────────────────────────────

/// Mortgage / housing loan details.
/// `monthly_principal`, `monthly_interest`, and `monthly_payment` are
/// **derived** — they are recomputed automatically whenever any input changes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Mortgage {
    /// Outstanding principal balance (残高) — input
    pub principal: i64,
    /// Annual interest rate as a decimal, e.g. 0.006 for 0.6% (年利) — input
    pub interest_rate: f64,
    /// Remaining term in months (残期間) — input
    pub remaining_months: u32,
    /// Monthly insurance / guarantee fee (保証料等) — input
    pub monthly_insurance: i64,
    /// Amortization method — input
    pub amortization: AmortizationMethod,

    // ── Computed (recalculated on every input change) ─────────────────────────
    pub monthly_principal: i64,
    pub monthly_interest:  i64,
    /// Exact annuity + insurance (nearest ¥1).
    pub monthly_total: i64,
    /// ¥100-rounded payment + insurance (matches dealer quotes).
    pub rounded_total: i64,
    /// First payment when using ¥100 rounding + insurance.
    pub first_payment: i64,
}

impl Mortgage {
    pub fn new() -> Self {
        let mut s = Self {
            principal: 0,
            interest_rate: 0.0,
            remaining_months: 0,
            monthly_insurance: 0,
            amortization: AmortizationMethod::default(),
            monthly_principal: 0,
            monthly_interest: 0,
            monthly_total: 0,
            rounded_total: 0,
            first_payment: 0,
        };
        s.recalculate();
        s
    }

    /// Recompute all derived fields from inputs.
    /// Call after changing: principal, interest_rate, remaining_months,
    /// monthly_insurance, or amortization.
    pub fn recalculate(&mut self) {
        let r = amort_calc(
            self.principal,
            self.interest_rate,
            self.remaining_months,
            self.amortization,
        );
        self.monthly_principal = r.monthly_principal;
        self.monthly_interest  = r.monthly_interest;
        self.monthly_total     = r.monthly_payment   + self.monthly_insurance;
        self.rounded_total     = r.rounded_payment   + self.monthly_insurance;
        self.first_payment     = r.first_payment     + self.monthly_insurance;
    }
}

impl Default for Mortgage {
    fn default() -> Self { Self::new() }
}

// ── Car loan ──────────────────────────────────────────────────────────────────

/// Car loan details.
/// Same auto-calculation approach as Mortgage, without insurance.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CarLoan {
    /// Outstanding principal balance (残高) — input
    pub principal: i64,
    /// Annual interest rate as a decimal (年利) — input
    pub interest_rate: f64,
    /// Remaining term in months (残期間) — input
    pub remaining_months: u32,
    /// Amortization method — input
    pub amortization: AmortizationMethod,

    // ── Computed ──────────────────────────────────────────────────────────────
    pub monthly_principal: i64,
    pub monthly_interest:  i64,
    /// Exact annuity (nearest ¥1).
    pub monthly_total:     i64,
    /// ¥100-rounded payment (matches dealer quotes).
    pub rounded_total:     i64,
    /// First payment when using ¥100 rounding.
    pub first_payment:     i64,
}

impl CarLoan {
    pub fn new() -> Self {
        let mut s = Self {
            principal: 0,
            interest_rate: 0.0,
            remaining_months: 0,
            amortization: AmortizationMethod::default(),
            monthly_principal: 0,
            monthly_interest: 0,
            monthly_total: 0,
            rounded_total: 0,
            first_payment: 0,
        };
        s.recalculate();
        s
    }

    pub fn recalculate(&mut self) {
        let r = amort_calc(
            self.principal,
            self.interest_rate,
            self.remaining_months,
            self.amortization,
        );
        self.monthly_principal = r.monthly_principal;
        self.monthly_interest  = r.monthly_interest;
        self.monthly_total     = r.monthly_payment;
        self.rounded_total     = r.rounded_payment;
        self.first_payment     = r.first_payment;
    }
}

impl Default for CarLoan {
    fn default() -> Self { Self::new() }
}

/// A single general debt / credit line.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Debt {
    pub label: String,
    pub principal: i64,
    pub monthly_payment: i64,
    pub interest_rate: f64,
    pub remaining_months: u32,
}

impl Debt {
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            principal: 0,
            monthly_payment: 0,
            interest_rate: 0.0,
            remaining_months: 0,
        }
    }
}

/// Top-level loans container.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Loans {
    pub mortgage: Mortgage,
    pub car: CarLoan,
    pub debts: Vec<Debt>,
}

impl Loans {
    pub fn total_monthly(&self) -> i64 {
        self.mortgage.monthly_total
            + self.car.monthly_total
            + self.debts.iter().map(|d| d.monthly_payment).sum::<i64>()
    }
}

impl Default for Loans {
    fn default() -> Self {
        Self {
            mortgage: Mortgage::default(),
            car: CarLoan::default(),
            debts: Vec::new(),
        }
    }
}

// ── Personal Expenses ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersonalExpenseItem {
    pub label: String,
    pub amount_a: i64,
    pub amount_b: i64,
}

impl PersonalExpenseItem {
    pub fn total(&self) -> i64 { self.amount_a + self.amount_b }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersonalExpenses {
    pub items: Vec<PersonalExpenseItem>,
}

impl PersonalExpenses {
    pub fn total(&self)   -> i64 { self.items.iter().map(|i| i.total()).sum() }
    pub fn total_a(&self) -> i64 { self.items.iter().map(|i| i.amount_a).sum() }
    pub fn total_b(&self) -> i64 { self.items.iter().map(|i| i.amount_b).sum() }
}



#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FamilyExpenseItem {
    pub label: String,
    pub total: i64,
    pub amount_a: i64,
    pub amount_b: i64,
}

impl FamilyExpenseItem {
    pub fn with_ratio(label: &str, total: i64, ratio_a: f64) -> Self {
        let amount_a = (total as f64 * ratio_a).round() as i64;
        Self { label: label.to_string(), total, amount_a, amount_b: total - amount_a }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FamilyExpenses {
    pub items: Vec<FamilyExpenseItem>,
}

impl FamilyExpenses {
    pub fn total(&self)   -> i64 { self.items.iter().map(|i| i.total).sum() }
    pub fn total_a(&self) -> i64 { self.items.iter().map(|i| i.amount_a).sum() }
    pub fn total_b(&self) -> i64 { self.items.iter().map(|i| i.amount_b).sum() }
}

// ── Other / Annual items ──────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OtherItem {
    pub label: String,
    pub annual_amount: i64,
    pub notes: String,
}

impl OtherItem {
    pub fn monthly_equivalent(&self) -> i64 { self.annual_amount / 12 }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OtherItems {
    pub items: Vec<OtherItem>,
}

impl OtherItems {
    pub fn total_annual(&self)         -> i64 { self.items.iter().map(|i| i.annual_amount).sum() }
    pub fn total_monthly_equiv(&self)  -> i64 { self.items.iter().map(|i| i.monthly_equivalent()).sum() }
}

// ── Full Budget ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Budget {
    pub month: String,
    pub income: Income,
    pub loans: Loans,
    pub personal_expenses: PersonalExpenses,
    pub family_expenses: FamilyExpenses,
    pub other_items: OtherItems,
    #[serde(default)]
    pub spending: SpendingLog,
}

/// Which income scenario is active.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum IncomeScenario {
    Normal,
    ParentalLeaveEarly,
    ParentalLeaveLate,
}

impl Budget {
    pub fn effective_income_total(&self, s: IncomeScenario) -> i64 {
        match s {
            IncomeScenario::Normal             => self.income.total_after_tax(),
            IncomeScenario::ParentalLeaveEarly => self.income.total_pl_early(),
            IncomeScenario::ParentalLeaveLate  => self.income.total_pl_late(),
        }
    }
    pub fn effective_income_a(&self, s: IncomeScenario) -> i64 {
        let m = self.income.members.first();
        match s {
            IncomeScenario::Normal             => m.map(|x| x.income_after_tax).unwrap_or(0),
            IncomeScenario::ParentalLeaveEarly => m.map(|x| x.parental_leave_early).unwrap_or(0),
            IncomeScenario::ParentalLeaveLate  => m.map(|x| x.parental_leave_late).unwrap_or(0),
        }
    }
    pub fn effective_income_b(&self, s: IncomeScenario) -> i64 {
        let m = self.income.members.get(1);
        match s {
            IncomeScenario::Normal             => m.map(|x| x.income_after_tax).unwrap_or(0),
            IncomeScenario::ParentalLeaveEarly => m.map(|x| x.parental_leave_early).unwrap_or(0),
            IncomeScenario::ParentalLeaveLate  => m.map(|x| x.parental_leave_late).unwrap_or(0),
        }
    }
    pub fn loan_total(&self, _s: IncomeScenario) -> i64 {
        self.loans.total_monthly()
    }
    pub fn balance_after_loans(&self, s: IncomeScenario) -> i64 {
        self.effective_income_total(s) - self.loan_total(s)
    }
    pub fn total_expenses(&self) -> i64 {
        self.family_expenses.total() + self.personal_expenses.total()
    }
    pub fn balance_final(&self, s: IncomeScenario) -> i64 {
        self.balance_after_loans(s) - self.total_expenses()
    }
    pub fn loan_a(&self, _s: IncomeScenario) -> i64 { 0 }
    pub fn loan_b(&self, s: IncomeScenario)  -> i64 { self.loan_total(s) }
    pub fn personal_tax_a(&self) -> i64 { 0 }
    pub fn personal_tax_b(&self) -> i64 { 0 }

    pub fn balance_final_a(&self, s: IncomeScenario) -> i64 {
        self.effective_income_a(s)
            - self.loan_a(s)
            - self.personal_tax_a()
            - self.family_expenses.total_a()
            - self.personal_expenses.total_a()
    }
    pub fn balance_final_b(&self, s: IncomeScenario) -> i64 {
        self.effective_income_b(s)
            - self.loan_b(s)
            - self.personal_tax_b()
            - self.family_expenses.total_b()
            - self.personal_expenses.total_b()
    }

    /// All budget category names (personal + family), for use in category picker.
    pub fn all_budget_categories(&self) -> Vec<(String, i64)> {
        let mut out: Vec<(String, i64)> = Vec::new();
        for item in &self.personal_expenses.items {
            out.push((item.label.clone(), item.total()));
        }
        for item in &self.family_expenses.items {
            out.push((item.label.clone(), item.total));
        }
        out.push(("Uncategorized".to_string(), 0));
        out
    }

    /// Rebuild spending categories from the current budget plan.
    /// Preserves existing transaction assignments.
    pub fn sync_spending_categories(&mut self) {
        self.spending.categories.clear();
        for item in &self.personal_expenses.items {
            self.spending.categories.push(SpendingCategory::new(&item.label, item.total()));
        }
        for item in &self.family_expenses.items {
            self.spending.categories.push(SpendingCategory::new(&item.label, item.total));
        }
        self.spending.categories.push(SpendingCategory::new("Uncategorized", 0));
    }
}

// ── Summary ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct Summary {
    pub income_total: i64,
    pub income_a: i64,
    pub income_b: i64,
    pub loan_payment: i64,
    pub family_expense_total: i64,
    pub family_expense_a: i64,
    pub family_expense_b: i64,
    pub personal_expense_total: i64,
    pub personal_expense_a: i64,
    pub personal_expense_b: i64,
    pub balance_total: i64,
    pub balance_a: i64,
    pub balance_b: i64,
}

impl Budget {
    pub fn summary(&self, s: IncomeScenario) -> Summary {
        Summary {
            income_total:           self.effective_income_total(s),
            income_a:               self.effective_income_a(s),
            income_b:               self.effective_income_b(s),
            loan_payment:           self.loan_total(s),
            family_expense_total:   self.family_expenses.total(),
            family_expense_a:       self.family_expenses.total_a(),
            family_expense_b:       self.family_expenses.total_b(),
            personal_expense_total: self.personal_expenses.total(),
            personal_expense_a:     self.personal_expenses.total_a(),
            personal_expense_b:     self.personal_expenses.total_b(),
            balance_total:          self.balance_final(s),
            balance_a:              self.balance_final_a(s),
            balance_b:              self.balance_final_b(s),
        }
    }
}

// ── Default budget (expense_ex.md values) ────────────────────────────────────

impl Default for Budget {
    fn default() -> Self {
        let month = chrono::Local::now().format("%Y-%m").to_string();
        Self {
            month,
            spending: SpendingLog::default(),
            income: Income {
                members: vec![
                    IncomeMember::new("A", 0, 0, 0),
                    IncomeMember::new("B", 0, 0, 0),
                ],
            },
            loans: Loans::default(),
            personal_expenses: PersonalExpenses {
                items: vec![
                    PersonalExpenseItem { label: "Phone".into(),           amount_a: 0, amount_b: 0 },
                    PersonalExpenseItem { label: "Cat insurance".into(),   amount_a: 0, amount_b: 0 },
                    PersonalExpenseItem { label: "NISA".into(),            amount_a: 0, amount_b: 0 },
                    PersonalExpenseItem { label: "Hair salon".into(),      amount_a: 0, amount_b: 0 },
                    PersonalExpenseItem { label: "Netflix".into(),         amount_a: 0, amount_b: 0 },
                    PersonalExpenseItem { label: "Cat food".into(),        amount_a: 0, amount_b: 0 },
                    PersonalExpenseItem { label: "Lunch".into(),           amount_a: 0, amount_b: 0 },
                    PersonalExpenseItem { label: "Saving".into(),          amount_a: 0, amount_b: 0 },
                    PersonalExpenseItem { label: "AI/Apple/Google".into(), amount_a: 0, amount_b: 0 },
                    PersonalExpenseItem { label: "Transportation".into(),  amount_a: 0, amount_b: 0 },
                    PersonalExpenseItem { label: "Others".into(),          amount_a: 0, amount_b: 0 },
                ],
            },
            family_expenses: FamilyExpenses {
                items: vec![
                    FamilyExpenseItem::with_ratio("Water",        0, 0.40),
                    FamilyExpenseItem::with_ratio("Electricity",  0, 0.40),
                    FamilyExpenseItem::with_ratio("Gas",          0, 0.40),
                    FamilyExpenseItem::with_ratio("Internet",     0, 0.40),
                    FamilyExpenseItem::with_ratio("Grocery",      0, 0.40),
                    FamilyExpenseItem::with_ratio("Cats",         0, 0.50),
                    FamilyExpenseItem::with_ratio("Eat out",      0, 0.50),
                    FamilyExpenseItem::with_ratio("Refrigerator", 0, 0.40),
                ],
            },
            other_items: OtherItems {
                items: vec![
                    OtherItem { label: "Property tax".into(),        annual_amount: 0, notes: "".into() },
                    OtherItem { label: "Car tax".into(),             annual_amount: 0, notes: "1,001cc~1,500cc".into() },
                    OtherItem { label: "Mortgage deduction".into(),  annual_amount: 0, notes: "".into() },
                    OtherItem { label: "Fire insurance (5y)".into(), annual_amount: 0, notes: "".into() },
                ],
            },
        }
    }
}
