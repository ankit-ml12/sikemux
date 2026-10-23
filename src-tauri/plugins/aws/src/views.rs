//! The views the AWS pane draws, worked out here from what the CLI answers so every client
//! draws the same thing.

use std::collections::HashMap;

use sikemux_plugin_api::aws::{Billing, BillingChange, BillingCharge, BillingMonth};

use crate::billing;
use crate::common::Target;
use crate::error::AwsResult;

const MONTH_NAMES: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];

/// Amounts under half a cent are nothing.
const NOTHING: f64 = 0.005;

pub(crate) async fn billing(target: &Target, months_back: u32) -> AwsResult<Billing> {
    let mut months = billing::months(target, months_back).await?;
    months.sort_by(|a, b| a.period_start.cmp(&b.period_start));
    Ok(billing_of(&months, time::OffsetDateTime::now_utc().day()))
}

/// The months Cost Explorer answered, oldest first, on day `today` (UTC) of the month in progress.
fn billing_of(raw: &[billing::BillingMonth], today: u8) -> Billing {
    let current = raw
        .iter()
        .position(|month| month.is_current)
        .unwrap_or(raw.len().saturating_sub(1));
    let befores = std::iter::once(None).chain(raw.iter().map(Some));
    let mut months: Vec<BillingMonth> = raw
        .iter()
        .zip(befores)
        .map(|(month, before)| month_of(month, before, today))
        .collect();
    let tallest = months.iter().map(|month| month.gross).fold(1., f64::max);
    for month in &mut months {
        month.ratio = month.gross / tallest;
    }
    let closed: Vec<f64> = months
        .iter()
        .filter(|month| !month.in_progress)
        .map(|month| month.gross)
        .collect();
    let last_closed = months.iter().rposition(|month| !month.in_progress);
    let average = (!closed.is_empty()).then(|| closed.iter().sum::<f64>() / closed.len() as f64);
    let forecast = months.get(current).map_or(0., |month| match month.day {
        Some(day) => month.gross / f64::from(day) * f64::from(month.days),
        None => month.gross,
    });
    let change_pct = closed
        .last()
        .copied()
        .filter(|before| *before > 0.)
        .map(|before| (forecast - before) / before * 100.);
    Billing {
        unit: raw
            .get(current)
            .map_or_else(|| "USD".to_owned(), |month| month.unit.clone()),
        current,
        forecast,
        last_closed,
        change_pct,
        average,
        average_ratio: average.map(|average| average / tallest),
        months,
    }
}

fn month_of(
    month: &billing::BillingMonth,
    before: Option<&billing::BillingMonth>,
    today: u8,
) -> BillingMonth {
    let amounts: Vec<(&str, f64)> = month
        .by_service
        .iter()
        .filter_map(|line| Some((line.service.as_str(), line.amount.parse::<f64>().ok()?)))
        .filter(|(_, amount)| amount.is_finite())
        .collect();
    let gross: f64 = amounts.iter().map(|(_, a)| a).filter(|a| **a >= 0.).sum();
    let credits: f64 = amounts.iter().map(|(_, a)| a).filter(|a| **a < 0.).sum();
    let (year, number) = year_month(&month.period_start);
    let days = number.map_or(30, |number| {
        time::Month::try_from(number).map_or(30, |name| u32::from(name.length(year)))
    });
    let day = month.is_current.then(|| u32::from(today).clamp(1, days));
    let pace = day.map_or(1., |day| f64::from(days) / f64::from(day));
    let earlier: Option<HashMap<&str, f64>> = before.map(|before| {
        let mut first = HashMap::new();
        for line in &before.by_service {
            let amount = line.amount.parse::<f64>().unwrap_or(f64::NAN);
            first.entry(line.service.as_str()).or_insert(amount);
        }
        first
    });
    let mut charges: Vec<(&str, f64)> = amounts
        .iter()
        .copied()
        .filter(|(_, a)| *a > NOTHING)
        .collect();
    charges.sort_by(|a, b| b.1.total_cmp(&a.1));
    let mut credit_lines: Vec<(&str, f64)> = amounts
        .iter()
        .copied()
        .filter(|(_, a)| *a < -NOTHING)
        .collect();
    credit_lines.sort_by(|a, b| a.1.total_cmp(&b.1));
    let top = charges.first().map_or(1., |(_, amount)| amount.max(1.));
    let line = |(service, amount): (&str, f64), change: Option<BillingChange>| BillingCharge {
        service: service.to_owned(),
        short_name: service
            .strip_prefix("Amazon ")
            .or_else(|| service.strip_prefix("AWS "))
            .unwrap_or(service)
            .to_owned(),
        amount,
        ratio: amount / top,
        change,
    };
    let (name, label, long_label) = names(&month.period_start);
    BillingMonth {
        period_start: month.period_start.clone(),
        label,
        name,
        long_label,
        in_progress: month.is_current,
        day,
        days,
        gross,
        credits,
        net: gross + credits,
        ratio: 0.,
        compared_to: before.map(|before| names(&before.period_start).1),
        charges: charges
            .into_iter()
            .map(|charge| {
                let change = earlier
                    .as_ref()
                    .map(|earlier| change_of(earlier.get(charge.0).copied(), charge.1 * pace));
                line(charge, change)
            })
            .collect(),
        credit_lines: credit_lines
            .into_iter()
            .map(|credit| line(credit, None))
            .collect(),
    }
}

/// How `now` moved from `before`, a percent either way counting as no move.
fn change_of(before: Option<f64>, now: f64) -> BillingChange {
    let Some(before) = before.filter(|before| before.is_finite() && *before > NOTHING) else {
        return BillingChange::New;
    };
    let pct = (now - before) / before * 100.;
    match pct {
        pct if pct.abs() < 1. => BillingChange::Flat,
        pct if pct > 0. => BillingChange::Up { pct },
        pct => BillingChange::Down { pct },
    }
}

fn year_month(period_start: &str) -> (i32, Option<u8>) {
    let mut parts = period_start.split('-');
    let year = parts
        .next()
        .and_then(|year| year.parse().ok())
        .unwrap_or(1970);
    let month = parts.next().and_then(|month| month.parse().ok());
    (year, month)
}

/// A month's name, as `Oct 26` and `October 2026`, from its first day.
fn names(period_start: &str) -> (String, String, String) {
    let (year, month) = year_month(period_start);
    let Some(name) = month
        .and_then(|month| usize::from(month).checked_sub(1))
        .and_then(|at| MONTH_NAMES.get(at))
    else {
        return (
            period_start.to_owned(),
            period_start.to_owned(),
            period_start.to_owned(),
        );
    };
    (
        (*name).to_owned(),
        format!(
            "{} {:02}",
            name.chars().take(3).collect::<String>(),
            year.rem_euclid(100)
        ),
        format!("{name} {year}"),
    )
}
