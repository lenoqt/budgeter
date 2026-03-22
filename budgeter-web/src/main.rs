//! Family Budget — web (WASM) entry point using webatui + Yew.
//!
//! This is a **read-only** dashboard that renders the same ratatui widgets as
//! the terminal app, but compiled to WebAssembly and displayed in a browser
//! via the webatui crate.  Editing is not supported in web mode — use the TUI
//! for data entry and export `budget.json` for the browser view.
//!
//! ## Data loading
//! The app tries to load `budget.json` from `localStorage` key `budget_data`.
//! If nothing is stored it falls back to the compiled-in DEFAULT_JSON constant
//! (an empty default budget).  When the TUI is running it can optionally write
//! a JSON export to localStorage so the web view stays in sync.
//!
//! ## Navigation
//! Tabs are clickable.  Scroll up / down changes the selected row in the
//! current tab.  A help panel is shown via the "?" button.

mod ui;

use budgeter_core::app::{App, Tab};
use budgeter_core::model::Budget;
use ratatui::prelude::*;
use webatui::prelude::*;
use yew::prelude::*;

// ── Default data (empty budget, shown when nothing is in localStorage) ────────

const DEFAULT_JSON: &str = r#"{
  "month": "2025-01",
  "income": { "members": [
    { "name": "A", "income_after_tax": 0, "parental_leave_early": 0, "parental_leave_late": 0 },
    { "name": "B", "income_after_tax": 0, "parental_leave_early": 0, "parental_leave_late": 0 }
  ]},
  "loans": {
    "mortgage": { "principal": 0, "interest_rate": 0.0, "remaining_months": 0, "monthly_insurance": 0, "amortization": "FixedPayment", "share_a": 0.5 },
    "car":      { "principal": 0, "interest_rate": 0.0, "remaining_months": 0, "amortization": "FixedPayment", "share_a": 0.5 },
    "debts": []
  },
  "personal_expenses": { "items": [] },
  "family_expenses":   { "items": [] },
  "other_items":       { "items": [] },
  "spending":          { "transactions": [], "categories": [] }
}"#;

// ── Messages ──────────────────────────────────────────────────────────────────

#[derive(Clone, PartialEq)]
pub enum Msg {
    /// User clicked a tab label (index into Tab::ALL)
    SelectTab(usize),
    /// Scroll down one row
    ScrollDown,
    /// Scroll up one row
    ScrollUp,
    /// Cycle income scenario (s)
    CycleScenario,
    /// Toggle help popup
    ToggleHelp,
    /// Close any popup
    ClosePopup,
}

// ── App wrapper ───────────────────────────────────────────────────────────────

/// Wrapper that holds the budgeter App state and implements TerminalApp.
#[derive(Clone, PartialEq)]
pub struct WebApp {
    inner: App,
}

impl WebApp {
    fn new() -> Self {
        let budget = load_budget_from_storage();
        let months = vec![budget.month.clone()];
        let inner = App::new(budget, months);
        Self { inner }
    }
}

impl TerminalApp for WebApp {
    type Message = Msg;

    fn update(&mut self, _ctx: TermContext<'_, Self>, msg: Self::Message) -> bool {
        match msg {
            Msg::SelectTab(idx) => {
                if let Some(tab) = Tab::ALL.get(idx) {
                    self.inner.active_tab = *tab;
                    self.inner.selected_row = 0;
                    true
                } else {
                    false
                }
            }
            Msg::ScrollDown => {
                self.inner.nav_row_down();
                true
            }
            Msg::ScrollUp => {
                self.inner.nav_row_up();
                true
            }
            Msg::CycleScenario => {
                self.inner.cycle_scenario();
                true
            }
            Msg::ToggleHelp => {
                use budgeter_core::app::Popup;
                self.inner.popup = match self.inner.popup {
                    Popup::Help => Popup::None,
                    _           => Popup::Help,
                };
                true
            }
            Msg::ClosePopup => {
                use budgeter_core::app::Popup;
                self.inner.popup = Popup::None;
                true
            }
        }
    }

    fn render(&self, _area: Rect, frame: &mut Frame<'_>) {
        // Clone so render can take &mut App for the existing draw function
        // (draw only mutates popup/scroll state which we don't need to persist
        //  in the read-only web version).
        let mut app_clone = self.inner.clone();
        ui::draw(frame, &mut app_clone);
    }

    fn hydrate(&self, ctx: &Context<WebTerminal<Self>>, span: &mut DehydratedSpan) {
        use budgeter_core::app::Popup;

        let text = span.text().to_string();

        // ── Tab click targets ────────────────────────────────────────────────
        for (idx, tab) in Tab::ALL.iter().enumerate() {
            let label = format!(" {} ", tab.title());
            if text.trim() == tab.title() || text == label {
                let cb = ctx.link().callback(move |_| Msg::SelectTab(idx));
                span.on_click(cb);
                return;
            }
        }

        // ── Scenario cycle button ────────────────────────────────────────────
        if text.contains("Scenario:") {
            let cb = ctx.link().callback(|_| Msg::CycleScenario);
            span.on_click(cb);
            return;
        }

        // ── Help button ──────────────────────────────────────────────────────
        if text == " ? " || text == "?" {
            let cb = ctx.link().callback(|_| Msg::ToggleHelp);
            span.on_click(cb);
            return;
        }

        // ── Close / Esc inside any popup ─────────────────────────────────────
        if matches!(self.inner.popup, Popup::Help) {
            if text.contains("Esc") || text.contains("close") || text.contains("✕") {
                let cb = ctx.link().callback(|_| Msg::ClosePopup);
                span.on_click(cb);
            }
        }
    }
}

// ── Entry point ───────────────────────────────────────────────────────────────

fn main() {
    run_tui(WebApp::new());
}

// ── localStorage helpers ──────────────────────────────────────────────────────

fn load_budget_from_storage() -> Budget {
    // Try to read from localStorage key "budget_data"
    if let Some(json) = read_local_storage("budget_data") {
        // The storage may contain a single Budget or a Vec<Budget>.
        // Try single first, then vec (take last).
        if let Ok(b) = serde_json::from_str::<Budget>(&json) {
            return b;
        }
        if let Ok(mut vec) = serde_json::from_str::<Vec<Budget>>(&json) {
            if let Some(last) = vec.pop() {
                return last;
            }
        }
    }

    // Fall back to compiled-in empty default
    serde_json::from_str::<Budget>(DEFAULT_JSON).unwrap_or_default()
}

fn read_local_storage(key: &str) -> Option<String> {
    use web_sys::window;
    let window = window()?;
    let storage = window.local_storage().ok()??;
    storage.get_item(key).ok()?
}
