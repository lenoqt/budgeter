//! Persistence layer — stores each month's Budget as a row in a Parquet file.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use polars::prelude::*;

use budgeter_core::model::{
    Budget, CarLoan, Debt, FamilyExpenseItem, FamilyExpenses, Income, IncomeMember, Loans,
    Mortgage, OtherItem, OtherItems, PersonalExpenseItem, PersonalExpenses, SpendingCategory,
    SpendingLog, Transaction,
};

pub struct Db {
    path: PathBuf,
}

impl Db {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    pub fn load_all(&self) -> Result<Vec<Budget>> {
        if !self.path.exists() {
            return Ok(vec![]);
        }
        let df = read_parquet(&self.path)?;
        rows_to_budgets(&df)
    }

    pub fn load_month(&self, month: &str) -> Result<Option<Budget>> {
        let all = self.load_all()?;
        Ok(all.into_iter().find(|b| b.month == month))
    }

    pub fn save(&self, budget: &Budget) -> Result<()> {
        let mut all = self.load_all()?;
        if let Some(pos) = all.iter().position(|b| b.month == budget.month) {
            all[pos] = budget.clone();
        } else {
            all.push(budget.clone());
        }
        all.sort_by(|a, b| a.month.cmp(&b.month));
        let df = budgets_to_df(&all)?;
        write_parquet(&self.path, df)?;
        Ok(())
    }

    pub fn delete_month(&self, month: &str) -> Result<()> {
        let mut all = self.load_all()?;
        all.retain(|b| b.month != month);
        let df = budgets_to_df(&all)?;
        write_parquet(&self.path, df)?;
        Ok(())
    }

    pub fn list_months(&self) -> Result<Vec<String>> {
        Ok(self.load_all()?.into_iter().map(|b| b.month).collect())
    }
}

// ── Internal helpers ──────────────────────────────────────────────────────────

fn read_parquet(path: &Path) -> Result<DataFrame> {
    let file = std::fs::File::open(path)
        .with_context(|| format!("Cannot open parquet file {}", path.display()))?;
    ParquetReader::new(file)
        .finish()
        .with_context(|| format!("Cannot read parquet file {}", path.display()))
}

fn write_parquet(path: &Path, mut df: DataFrame) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let file = std::fs::File::create(path)
        .with_context(|| format!("Cannot create parquet file {}", path.display()))?;
    ParquetWriter::new(file)
        .finish(&mut df)
        .with_context(|| "Cannot write parquet file")?;
    Ok(())
}

// ── Budget → DataFrame ────────────────────────────────────────────────────────

fn budgets_to_df(budgets: &[Budget]) -> Result<DataFrame> {
    let mut months:        Vec<String> = Vec::new();
    let mut members_json:  Vec<String> = Vec::new();
    let mut inc_a_after:   Vec<i64>    = Vec::new();
    let mut inc_a_early:   Vec<i64>    = Vec::new();
    let mut inc_a_late:    Vec<i64>    = Vec::new();
    let mut inc_b_after:   Vec<i64>    = Vec::new();
    let mut inc_b_early:   Vec<i64>    = Vec::new();
    let mut inc_b_late:    Vec<i64>    = Vec::new();
    let mut mortgage_json: Vec<String> = Vec::new();
    let mut car_json:      Vec<String> = Vec::new();
    let mut debts_json:    Vec<String> = Vec::new();
    let mut personal_json: Vec<String> = Vec::new();
    let mut family_json:   Vec<String> = Vec::new();
    let mut other_json:    Vec<String> = Vec::new();
    let mut spending_json: Vec<String> = Vec::new();

    for b in budgets {
        let a = b.income.members.first().cloned()
            .unwrap_or_else(|| IncomeMember::new("A", 0, 0, 0));
        let g = b.income.members.get(1).cloned()
            .unwrap_or_else(|| IncomeMember::new("B", 0, 0, 0));

        let names: Vec<&str> = b.income.members.iter().map(|m| m.name.as_str()).collect();

        months.push(b.month.clone());
        members_json.push(serde_json::to_string(&names).unwrap_or_default());
        inc_a_after.push(a.income_after_tax);
        inc_a_early.push(a.parental_leave_early);
        inc_a_late.push(a.parental_leave_late);
        inc_b_after.push(g.income_after_tax);
        inc_b_early.push(g.parental_leave_early);
        inc_b_late.push(g.parental_leave_late);
        mortgage_json.push(serde_json::to_string(&b.loans.mortgage).unwrap_or_default());
        car_json.push(serde_json::to_string(&b.loans.car).unwrap_or_default());
        debts_json.push(serde_json::to_string(&b.loans.debts).unwrap_or_default());
        personal_json.push(serde_json::to_string(&b.personal_expenses.items).unwrap_or_default());
        family_json.push(serde_json::to_string(&b.family_expenses.items).unwrap_or_default());
        other_json.push(serde_json::to_string(&b.other_items.items).unwrap_or_default());

        let spending_blob = SpendingBlob {
            transactions: b.spending.transactions.clone(),
            categories:   b.spending.categories.clone(),
        };
        spending_json.push(serde_json::to_string(&spending_blob).unwrap_or_default());
    }

    let cols: Vec<Column> = vec![
        Column::new("month".into(),         months),
        Column::new("members_json".into(),  members_json),
        Column::new("inc_a_after".into(),   inc_a_after),
        Column::new("inc_a_early".into(),   inc_a_early),
        Column::new("inc_a_late".into(),    inc_a_late),
        Column::new("inc_b_after".into(),   inc_b_after),
        Column::new("inc_b_early".into(),   inc_b_early),
        Column::new("inc_b_late".into(),    inc_b_late),
        Column::new("mortgage_json".into(), mortgage_json),
        Column::new("car_json".into(),      car_json),
        Column::new("debts_json".into(),    debts_json),
        Column::new("personal_json".into(), personal_json),
        Column::new("family_json".into(),   family_json),
        Column::new("other_json".into(),    other_json),
        Column::new("spending_json".into(), spending_json),
    ];
    Ok(unsafe { DataFrame::new_unchecked_infer_height(cols) })
}

// ── DataFrame → Budget ────────────────────────────────────────────────────────

fn rows_to_budgets(df: &DataFrame) -> Result<Vec<Budget>> {
    let n = df.height();

    let months        = str_col(df, "month")?;
    let members_jsn   = str_col(df, "members_json")?;
    let a_after_tax   = i64_col(df, "inc_a_after")?;
    let a_pl_early    = i64_col(df, "inc_a_early")?;
    let a_pl_late     = i64_col(df, "inc_a_late")?;
    let b_after_tax   = i64_col(df, "inc_b_after")?;
    let b_pl_early    = i64_col(df, "inc_b_early")?;
    let b_pl_late     = i64_col(df, "inc_b_late")?;
    let personal_jsn  = str_col(df, "personal_json")?;
    let family_jsn    = str_col(df, "family_json")?;
    let other_jsn     = str_col(df, "other_json")?;

    let col_names: Vec<&str> = df.get_column_names().iter().map(|s| s.as_str()).collect();

    // Helper: return a str column if present, otherwise a vec of None (backwards compat).
    macro_rules! optional_str_col {
        ($name:expr) => {
            if col_names.contains(&$name) {
                str_col(df, $name)?
            } else {
                vec![None; n]
            }
        };
    }

    let mortgage_jsn  = optional_str_col!("mortgage_json");
    let car_jsn       = optional_str_col!("car_json");
    let debts_jsn     = optional_str_col!("debts_json");
    let spending_jsn  = optional_str_col!("spending_json");

    let mut out = Vec::with_capacity(n);

    for i in 0..n {
        let month = months[i].unwrap_or("").to_string();

        let names: Vec<String> = members_jsn[i]
            .and_then(|s| serde_json::from_str(s).ok())
            .unwrap_or_else(|| vec!["A".into(), "B".into()]);

        let name_a = names.first().cloned().unwrap_or_else(|| "A".into());
        let name_b = names.get(1).cloned().unwrap_or_else(|| "B".into());

        let income = Income {
            members: vec![
                IncomeMember::new(&name_a, a_after_tax[i], a_pl_early[i], a_pl_late[i]),
                IncomeMember::new(&name_b, b_after_tax[i], b_pl_early[i], b_pl_late[i]),
            ],
        };

        let personal_items: Vec<PersonalExpenseItem> = personal_jsn[i]
            .and_then(|s| serde_json::from_str(s).ok())
            .unwrap_or_default();

        let family_items: Vec<FamilyExpenseItem> = family_jsn[i]
            .and_then(|s| serde_json::from_str(s).ok())
            .unwrap_or_default();

        let other_items: Vec<OtherItem> = other_jsn[i]
            .and_then(|s| serde_json::from_str(s).ok())
            .unwrap_or_default();

        let mut mortgage: Mortgage = mortgage_jsn[i]
            .and_then(|s| serde_json::from_str(s).ok())
            .unwrap_or_default();
        mortgage.recalculate();

        let mut car: CarLoan = car_jsn[i]
            .and_then(|s| serde_json::from_str(s).ok())
            .unwrap_or_default();
        car.recalculate();

        let debts: Vec<Debt> = debts_jsn[i]
            .and_then(|s| serde_json::from_str(s).ok())
            .unwrap_or_default();

        let spending: SpendingLog = spending_jsn[i]
            .and_then(|s| serde_json::from_str::<SpendingBlob>(s).ok())
            .map(|blob| SpendingLog {
                transactions: blob.transactions,
                categories:   blob.categories,
            })
            .unwrap_or_default();

        out.push(Budget {
            month,
            income,
            loans: Loans { mortgage, car, debts },
            personal_expenses: PersonalExpenses { items: personal_items },
            family_expenses:   FamilyExpenses   { items: family_items   },
            other_items:       OtherItems        { items: other_items    },
            spending,
        });
    }

    Ok(out)
}

// ── Spending blob (intermediate serde struct) ─────────────────────────────────

/// Helper that groups both halves of a SpendingLog into one JSON blob.
#[derive(serde::Serialize, serde::Deserialize)]
struct SpendingBlob {
    transactions: Vec<Transaction>,
    categories:   Vec<SpendingCategory>,
}

// ── Column extraction helpers ─────────────────────────────────────────────────

/// Returns a Vec of `Option<&str>` — one entry per row.
fn str_col<'a>(df: &'a DataFrame, name: &str) -> Result<Vec<Option<&'a str>>> {
    Ok(df
        .column(name)
        .with_context(|| format!("Missing column '{name}'"))?
        .str()
        .with_context(|| format!("Column '{name}' is not Utf8"))?
        .into_iter()
        .collect())
}

fn i64_col(df: &DataFrame, name: &str) -> Result<Vec<i64>> {
    Ok(df
        .column(name)
        .with_context(|| format!("Missing column '{name}'"))?
        .i64()
        .with_context(|| format!("Column '{name}' is not Int64"))?
        .into_iter()
        .map(|v| v.unwrap_or(0))
        .collect())
}
