//! The views the AWS pane draws, worked out here from what the CLI answers so every client
//! draws the same thing.

use std::collections::HashMap;

use sikemux_plugin_api::aws::{
    Billing, BillingChange, BillingCharge, BillingMonth, Ec2, Ec2Instance, Health, Lambda,
    LambdaFunction, RuntimeFamily, S3Bucket, Sqs, SqsQueue, S3,
};
use sikemux_plugin_api::PluginContext;

use crate::common::Target;
use crate::error::AwsResult;
use crate::{auth, billing, ec2, lambda, s3, sqs};

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

/// Where the console opens when a profile names no region.
const FALLBACK_REGION: &str = "us-east-1";

const DEPRECATED_RUNTIMES: [&str; 11] = [
    "python3.7",
    "python3.8",
    "python3.9",
    "nodejs12.x",
    "nodejs14.x",
    "nodejs16.x",
    "nodejs18.x",
    "go1.x",
    "java8",
    "dotnet6",
    "ruby2.7",
];

const RUNTIME_FAMILIES: [(&str, &str); 7] = [
    ("nodejs", "Node"),
    ("python", "Python"),
    ("java", "Java"),
    ("go", "Go"),
    ("dotnet", ".NET"),
    ("ruby", "Ruby"),
    ("provided", "Custom"),
];

/// The smallest share of a bar a queue holding anything gets, so it still shows.
const SLIVER: f64 = 0.03;

/// The profile's own region, which is the one its calls run in.
async fn targeted(ctx: &PluginContext, asked: Target) -> AwsResult<(Target, String)> {
    let target = auth::resolve(ctx, asked).await?;
    let region = target
        .region
        .clone()
        .unwrap_or_else(|| FALLBACK_REGION.to_owned());
    Ok((target, region))
}

pub(crate) async fn ec2(ctx: &PluginContext, asked: Target) -> AwsResult<Ec2> {
    let (target, region) = targeted(ctx, asked).await?;
    let instances: Vec<Ec2Instance> = ec2::instances(&target)
        .await?
        .into_iter()
        .map(|instance| instance_of(instance, &region))
        .collect();
    Ok(Ec2 {
        running: instances
            .iter()
            .filter(|instance| instance.state.as_deref() == Some("running"))
            .count() as u32,
        instances,
        region,
    })
}

fn instance_of(instance: ec2::Ec2Instance, region: &str) -> Ec2Instance {
    let (health, label) = match instance.state.as_deref() {
        Some("running") => (Health::Ok, "Running"),
        Some("pending") => (Health::Warn, "Starting"),
        Some("stopping") => (Health::Warn, "Stopping"),
        Some("shutting-down") => (Health::Warn, "Shutting down"),
        Some("stopped") => (Health::Off, "Stopped"),
        Some("terminated") => (Health::Off, "Terminated"),
        Some(other) => (Health::Off, other),
        None => (Health::Off, "Unknown"),
    };
    Ec2Instance {
        state_label: label.to_owned(),
        health,
        idle: matches!(instance.state.as_deref(), Some("stopped" | "terminated")),
        console_url: format!(
            "https://{region}.console.aws.amazon.com/ec2/home?region={region}#InstanceDetails:instanceId={}",
            instance.instance_id
        ),
        id: instance.instance_id,
        name: instance.name,
        state: instance.state,
        instance_type: instance.instance_type,
        private_ip: instance.private_ip,
        public_ip: instance.public_ip,
        launched_at: instance.launch_time,
    }
}

pub(crate) async fn lambda(ctx: &PluginContext, asked: Target) -> AwsResult<Lambda> {
    let (target, region) = targeted(ctx, asked).await?;
    let raw = lambda::functions(&target).await?;
    let most = raw
        .iter()
        .filter_map(|function| function.memory_size)
        .fold(1024, i64::max);
    let functions: Vec<LambdaFunction> = raw
        .into_iter()
        .map(|function| function_of(function, most, &region))
        .collect();
    let mut families: Vec<RuntimeFamily> = Vec::new();
    for function in &functions {
        match families
            .iter_mut()
            .find(|family| family.key == function.family)
        {
            Some(family) => family.count += 1,
            None => families.push(RuntimeFamily {
                key: function.family.clone(),
                label: family_label(&function.family).to_owned(),
                count: 1,
            }),
        }
    }
    families.sort_by_key(|family| std::cmp::Reverse(family.count));
    Ok(Lambda {
        region,
        families,
        functions,
    })
}

fn family_label(key: &str) -> &'static str {
    RUNTIME_FAMILIES
        .iter()
        .find(|(family, _)| *family == key)
        .map_or("Other", |(_, label)| *label)
}

fn function_of(function: lambda::LambdaFn, most_memory: i64, region: &str) -> LambdaFunction {
    let family = function
        .runtime
        .as_deref()
        .and_then(|runtime| {
            RUNTIME_FAMILIES
                .iter()
                .find(|(family, _)| runtime.starts_with(family))
        })
        .map_or("other", |(family, _)| *family);
    let memory_mb = function
        .memory_size
        .and_then(|mb| u32::try_from(mb).ok())
        .filter(|mb| *mb > 0);
    LambdaFunction {
        family: family.to_owned(),
        deprecated: function
            .runtime
            .as_deref()
            .is_some_and(|runtime| DEPRECATED_RUNTIMES.contains(&runtime)),
        memory_label: memory_mb.map_or_else(|| "—".to_owned(), memory_label),
        memory_ratio: memory_mb.map_or(0., |mb| f64::from(mb) / most_memory as f64),
        memory_mb,
        timeout_secs: function.timeout.and_then(|secs| u32::try_from(secs).ok()),
        handler: function.handler,
        modified_at: function.last_modified,
        log_group: format!("/aws/lambda/{}", function.name),
        console_url: format!(
            "https://{region}.console.aws.amazon.com/lambda/home?region={region}#/functions/{}",
            encoded(&function.name)
        ),
        runtime: function.runtime,
        name: function.name,
    }
}

/// `512 MB`, or `1.5 GB` from a gigabyte up.
fn memory_label(mb: u32) -> String {
    if mb < 1024 {
        return format!("{mb} MB");
    }
    let gb = format!("{:.1}", f64::from(mb) / 1024.);
    format!("{} GB", gb.strip_suffix(".0").unwrap_or(&gb))
}

pub(crate) async fn sqs(ctx: &PluginContext, asked: Target) -> AwsResult<Sqs> {
    let (target, region) = targeted(ctx, asked).await?;
    let raw = sqs::queues(&target).await?;
    let count = |value: &Option<String>| value.as_deref().and_then(|n| n.parse::<u64>().ok());
    let most = raw
        .iter()
        .filter_map(|queue| count(&queue.messages))
        .fold(1, u64::max);
    let queues: Vec<SqsQueue> = raw
        .into_iter()
        .map(|queue| {
            let available = count(&queue.messages);
            let dead_letter = is_dead_letter(&queue.name);
            SqsQueue {
                fifo: queue.name.ends_with(".fifo"),
                dead_letter,
                ratio: available
                    .filter(|n| *n > 0)
                    .map_or(0., |n| (n as f64 / most as f64).sqrt().max(SLIVER)),
                needs_look: dead_letter && available.is_some_and(|n| n > 0),
                in_flight: count(&queue.in_flight),
                delayed: count(&queue.delayed),
                available,
                console_url: format!(
                    "https://{region}.console.aws.amazon.com/sqs/v3/home?region={region}#/queues/{}",
                    encoded(&queue.url)
                ),
                name: queue.name,
                url: queue.url,
            }
        })
        .collect();
    Ok(Sqs {
        region,
        dead_letters: queues.iter().filter(|queue| queue.dead_letter).count() as u32,
        queues,
    })
}

/// Named as a dead-letter queue: `-dlq`, `_dlq`, `-dead-letter` or `-deadletter` at its end.
fn is_dead_letter(name: &str) -> bool {
    let name = name.to_lowercase();
    let name = name.strip_suffix(".fifo").unwrap_or(&name);
    ["dlq", "dead-letter", "deadletter"].iter().any(|ending| {
        name.strip_suffix(ending)
            .is_some_and(|rest| rest.ends_with('-') || rest.ends_with('_'))
    })
}

pub(crate) async fn s3(ctx: &PluginContext, asked: Target) -> AwsResult<S3> {
    let (target, region) = targeted(ctx, asked).await?;
    let buckets = s3::buckets(&target)
        .await?
        .into_iter()
        .map(|bucket| S3Bucket {
            uri: format!("s3://{}", bucket.name),
            arn: format!("arn:aws:s3:::{}", bucket.name),
            console_url: format!(
                "https://s3.console.aws.amazon.com/s3/buckets/{}",
                encoded(&bucket.name)
            ),
            name: bucket.name,
            created_at: bucket.created_at,
        })
        .collect();
    Ok(S3 { region, buckets })
}

/// `text` made safe to put in a URL, as a browser's `encodeURIComponent` makes it.
fn encoded(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for byte in text.bytes() {
        if byte.is_ascii_alphanumeric() || b"-_.!~*'()".contains(&byte) {
            out.push(char::from(byte));
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

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
