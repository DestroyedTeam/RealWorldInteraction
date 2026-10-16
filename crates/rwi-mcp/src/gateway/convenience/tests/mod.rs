use super::*;
use std::cell::RefCell;
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct LegacyResolveView {
    target: String,
    view_id: String,
    x: f64,
    y: f64,
}

mod click_delivery;
mod controls;
mod drag_between;
mod hover;
mod inputs;
mod receipts;
mod schemas;
mod snapshots;
