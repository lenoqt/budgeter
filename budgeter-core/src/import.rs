//! CSV import logic for credit card statements.
//!
//! Each provider gets its own parse function.
//! All parsing is done with pure std — no extra CSV crates needed.
//! Rakuten Card exports Shift-JIS; we use encoding_rs to transcode.

use anyhow::{bail, Context, Result};

use crate::model::{CardProvider, Transaction};

// ── Public entry point ────────────────────────────────────────────────────────

/// Parse a card statement CSV file and return raw transactions.
/// All transactions start with category = "" (empty = not yet categorized).
pub fn parse_csv(path: &str, provider: CardProvider) -> Result<Vec<Transaction>> {
    let bytes = std::fs::read(path)
        .with_context(|| format!("Cannot read file: {path}"))?;

    match provider {
        CardProvider::RakutenCard => parse_rakuten(&bytes),
    }
}

// ── Rakuten Card ──────────────────────────────────────────────────────────────
//
// Column layout (0-based index):
//   0  利用日               purchase date        YYYY/MM/DD or empty
//   1  利用店名・商品名      merchant name
//   2  利用者               cardholder           本人 = primary
//   3  支払方法             payment method
//   4  利用金額             original amount
//   5  手数料/利息          fee / interest
//   6  支払総額             total payable
//   7  支払月               payment month label  e.g. "4月"
//   8  N月支払金額          amount due this stmt (dynamic column header)
//   9  N月繰越残高          carryover balance
//  10  N月以降支払金額      future installment balance
//
// Rules:
//   - File is encoded in Shift-JIS → decode with encoding_rs.
//   - First row is the header — skip it.
//   - Skip any row where column 0 (date) is empty (continuation note rows).
//   - Skip rows where column 7 (支払月) contains "以降" — future installments
//     we don't count yet. Only rows whose payment month matches the current
//     statement are relevant.
//   - Amount this month = column 8 (parse as i64, ignore commas/¥).

fn parse_rakuten(bytes: &[u8]) -> Result<Vec<Transaction>> {
    // ── Transcode Shift-JIS → UTF-8 ───────────────────────────────────────────
    let (cow, _encoding, _had_errors) = encoding_rs::SHIFT_JIS.decode(bytes);
    let text = cow.as_ref();

    // ── Parse CSV lines manually ───────────────────────────────────────────────
    let mut transactions = Vec::new();

    for (line_no, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        let cols = parse_csv_line(line);

        // Skip header
        if line_no == 0 {
            continue;
        }

        // Must have at least 9 columns
        if cols.len() < 9 {
            continue;
        }

        // Skip continuation/note rows (column 0 date is empty)
        let date_raw = cols[0].trim();
        if date_raw.is_empty() {
            continue;
        }

        // Skip rows whose payment is "N月以降" (future installments)
        let payment_month = cols.get(7).map(|s| s.trim()).unwrap_or("");
        if payment_month.contains("以降") {
            continue;
        }

        let date          = normalize_date(date_raw);
        let merchant      = cols[1].trim().to_string();
        let cardholder    = cols[2].trim().to_string();
        let payment_meth  = cols[3].trim().to_string();
        let amount        = parse_jpy_col(cols.get(4).copied().unwrap_or(""));
        let fee           = parse_jpy_col(cols.get(5).copied().unwrap_or(""));
        let amount_this_month = parse_jpy_col(cols.get(8).copied().unwrap_or(""));

        // Skip truly empty rows
        if amount == 0 && amount_this_month == 0 {
            continue;
        }

        transactions.push(Transaction {
            date,
            merchant,
            cardholder,
            payment_method: payment_meth,
            amount,
            fee,
            amount_this_month,
            category: String::new(),
            provider: "Rakuten Card".to_string(),
            member: String::new(),
        });
    }

    if transactions.is_empty() {
        bail!("No transactions found. Check that the file is a Rakuten Card CSV.");
    }

    Ok(transactions)
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Minimal RFC-4180 CSV field parser for a single line.
fn parse_csv_line(line: &str) -> Vec<&str> {
    let mut fields = Vec::new();
    let bytes = line.as_bytes();
    let mut i = 0;

    while i <= bytes.len() {
        if i < bytes.len() && bytes[i] == b'"' {
            let start = i + 1;
            let mut end = start;
            while end < bytes.len() {
                if bytes[end] == b'"' {
                    if end + 1 < bytes.len() && bytes[end + 1] == b'"' {
                        end += 2;
                    } else {
                        break;
                    }
                } else {
                    end += 1;
                }
            }
            fields.push(&line[start..end]);
            i = end + 1;
            if i < bytes.len() && bytes[i] == b',' {
                i += 1;
            }
        } else {
            let start = i;
            while i < bytes.len() && bytes[i] != b',' {
                i += 1;
            }
            fields.push(&line[start..i]);
            if i < bytes.len() { i += 1; } else { i += 1; }
        }
    }

    fields
}

fn parse_jpy_col(s: &str) -> i64 {
    s.chars().filter(|c| c.is_ascii_digit() || *c == '-').collect::<String>()
        .parse().unwrap_or(0)
}

fn normalize_date(s: &str) -> String {
    s.replace('/', "-")
}
