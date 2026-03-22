# 家族予算 Family Budget

A dual-mode family budget application built in Rust:
- **Terminal (TUI)** — full interactive editor with keyboard navigation
- **Web** — read-only browser dashboard powered by WebAssembly

---

## Project layout

```
Budgeter/
├── budgeter-core/        # Shared data model, app state, CSV import logic
├── budgeter-tui/         # Terminal binary (ratatui 0.30 + crossterm + polars)
├── budgeter-web/         # WASM binary  (ratatui 0.25 + webatui + Yew)
│   ├── index.html        # Trunk entry point
│   └── Trunk.toml        # Trunk build/serve config
├── budget.parquet        # Persistent store (written by TUI)
└── budget.json           # JSON export     (auto-written by TUI on every save)
```

---

## Running the Terminal UI

```bash
# From the workspace root (WSL / Linux / macOS)
cargo run -p budgeter-tui

# Or build a release binary
cargo build --release -p budgeter-tui
./target/release/budgeter-tui
```

### Keyboard shortcuts

| Key | Action |
|-----|--------|
| `Tab` / `Shift-Tab` | Next / previous tab |
| `↑` `↓` `j` `k` | Move row |
| `←` `→` `h` `l` | Move column |
| `Enter` / `e` | Edit selected cell |
| `Esc` | Cancel edit / close popup |
| `a` | Add row |
| `d` | Delete row |
| `s` | Cycle income scenario |
| `S` | Save |
| `q` | Quit (auto-saves if dirty) |
| `m` | Month picker |
| `n` | New month |
| `?` | Help popup |
| **Import tab** | |
| `P` | Parse CSV file |
| `C` | Commit parsed transactions |
| `X` | Clear preview |
| `W` | Bulk-assign member to all rows |
| `m` | Assign member to selected row |
| **Spending tab** | |
| `Enter` | Drill into category |
| `Esc` | Exit drill-down |
| `e` | Reassign category (drill view) |
| `m` | Reassign member (drill view) |

### Data files

- **`budget.parquet`** — primary Parquet store; one row per month.
- **`budget.json`** — JSON export written automatically on every save.
  The web viewer reads this file; keep it alongside `budget.parquet`.

---

## Running the Web Viewer

The web viewer is a **read-only** dashboard.  It renders the same UI as
the TUI (Income, Loans, Spending, Charts, Summary…) in the browser using
WebAssembly via [webatui](https://crates.io/crates/webatui) + [Yew](https://yew.rs).

### Prerequisites

```bash
# 1. Install the WASM compilation target (once)
rustup target add wasm32-unknown-unknown

# 2. Install Trunk (the WASM bundler / dev server)
cargo install --locked trunk
```

### Development server (hot-reload)

```bash
cd budgeter-web
trunk serve          # starts on http://127.0.0.1:8080
```

Open `http://127.0.0.1:8080` in your browser.

### Production build

```bash
cd budgeter-web
trunk build --release
# Output is in budgeter-web/dist/ — serve with any static file server
```

### Loading your budget data in the browser

The web app reads budget data from the browser's **localStorage** under the
key `budget_data`.  The TUI writes `budget.json` on every save — paste its
contents into localStorage to sync:

1. Run the TUI, make changes, press `S` to save.
2. Open `budget.json` in a text editor and copy its contents.
3. Open the browser DevTools (`F12`).
4. Go to **Application → Storage → Local Storage → http://127.0.0.1:8080**.
5. Add / update the key `budget_data` with the copied JSON.
6. Reload the page.

> **Tip:** If `budget.json` contains a JSON array of months, the web viewer
> automatically picks the last entry.

#### One-liner (browser console)

```js
// Paste this in the DevTools console after copying budget.json contents
localStorage.setItem('budget_data', /* paste JSON string here */);
location.reload();
```

---

## Architecture

```
┌──────────────────────────────────────────────────┐
│               budgeter-core                       │
│  model.rs   app.rs   import.rs                    │
│  (no ratatui, no crossterm, no polars)            │
└────────────────┬──────────────────────────────────┘
                 │  shared as a library crate
       ┌─────────┴──────────┐
       │                    │
┌──────▼──────┐    ┌────────▼──────────┐
│budgeter-tui │    │  budgeter-web     │
│             │    │                   │
│ ratatui 0.30│    │ ratatui 0.25      │
│ crossterm   │    │ webatui 0.1.1     │
│ polars/     │    │ Yew 0.21          │
│  parquet    │    │ wasm32 target     │
│             │    │                   │
│ Full editor │    │ Read-only viewer  │
│ ← keyboard  │    │ ← click tabs      │
│   editing   │    │   scroll rows     │
└─────────────┘    └───────────────────┘
      ↓ saves             ↑ reads
  budget.parquet   budget.json (auto-exported)
```

---

## Web mode limitations

Because the web target is WebAssembly running in a browser sandbox:

| Feature | TUI | Web |
|---------|-----|-----|
| View all tabs | ✅ | ✅ |
| Click to switch tabs | — | ✅ |
| Scroll rows | ✅ | ✅ |
| Edit values | ✅ | ❌ |
| Import CSV | ✅ | ❌ |
| Save to disk | ✅ | ❌ |
| Parquet persistence | ✅ | ❌ |
| Charts | ✅ | ✅ |

Keyboard editing in the browser is not yet supported by `webatui 0.1.1`.
Use the TUI for all data entry.

---

## Building both at once

```bash
# Build the TUI (native)
cargo build -p budgeter-tui

# Check the web crate compiles for WASM
cargo check -p budgeter-web --target wasm32-unknown-unknown

# Full WASM bundle (run from budgeter-web/)
cd budgeter-web && trunk build
```

---

## CSV Import (Rakuten Card)

1. Log into Rakuten Card online and download your statement as CSV.
2. In the TUI, go to the **Import** tab.
3. Enter the file path (Shift-JIS encoded; the app decodes automatically).
4. Press `P` to parse, assign categories with `Enter`, then `C` to commit.
5. Press `S` to save — `budget.json` is updated automatically for the web viewer.