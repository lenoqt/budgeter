//! budgeter-core — shared data-model, application state, and import logic.
//!
//! This crate has **no** rendering dependency (no ratatui, no crossterm,
//! no polars).  Both the terminal (`budgeter-tui`) and the browser
//! (`budgeter-web`) frontends depend on this crate.

pub mod app;
pub mod import;
pub mod model;
