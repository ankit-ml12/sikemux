//! What the AWS plugin's views hand a pane: an account's costs by month.

use serde::{Deserialize, Serialize};

/// An account's costs over the last months, worked out for drawing: each month's charges and
/// credits, and how the month in progress is heading.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Billing {
    /// The currency every amount is in, such as `USD`.
    pub unit: String,
    /// Oldest first.
    pub months: Vec<BillingMonth>,
    /// The place in `months` of the month in progress, else of the newest.
    pub current: usize,
    /// What the current month comes to at the pace it has kept; its charges so far once closed.
    pub forecast: f64,
    /// The place in `months` of the newest closed month.
    pub last_closed: Option<usize>,
    /// How far the forecast is above or below the last closed month's charges, in percent.
    pub change_pct: Option<f64>,
    /// The closed months' charges on average, and that over the tallest month's.
    pub average: Option<f64>,
    pub average_ratio: Option<f64>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BillingMonth {
    /// `YYYY-MM-01`.
    pub period_start: String,
    /// `Oct 26`.
    pub label: String,
    /// `October`.
    pub name: String,
    /// `October 2026`.
    pub long_label: String,
    pub in_progress: bool,
    /// For the month in progress, Cost Explorer's day (UTC) and the month's days.
    pub day: Option<u32>,
    pub days: u32,
    /// Its charges before credits, its credits (negative), and the two together.
    pub gross: f64,
    pub credits: f64,
    pub net: f64,
    /// Its charges over the tallest month's.
    pub ratio: f64,
    /// The month before, which each charge's change is against.
    pub compared_to: Option<String>,
    /// Largest first.
    pub charges: Vec<BillingCharge>,
    /// Largest first.
    pub credit_lines: Vec<BillingCharge>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BillingCharge {
    pub service: String,
    /// The service without its `Amazon` or `AWS` in front.
    pub short_name: String,
    pub amount: f64,
    /// Its amount over the month's largest charge.
    pub ratio: f64,
    pub change: Option<BillingChange>,
}

/// How a service's charge moved from the month before; for the month in progress, at its pace.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum BillingChange {
    /// Nothing was charged for it the month before.
    New,
    /// Within a percent either way.
    Flat,
    Up {
        pct: f64,
    },
    Down {
        pct: f64,
    },
}
