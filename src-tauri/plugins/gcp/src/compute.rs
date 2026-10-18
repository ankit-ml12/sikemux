// Compute Engine: VMs in every zone, starting, stopping and resetting them,
// their serial console output, and the command that opens a shell on one.

use serde::Deserialize;
use serde_json::{json, Value};

use crate::auth::Target;
use crate::client::{self, at, segment, short, text};
use crate::error::{GcpError, GcpResult};

const API: &str = "https://compute.googleapis.com/compute/v1";
const MAX_PAGES: usize = 10;
const MAX_SERIAL_BYTES: usize = 256 * 1024;

fn ips(instance: &Value) -> (Vec<String>, Vec<String>) {
    let mut internal = Vec::new();
    let mut external = Vec::new();
    for interface in at(instance, "/networkInterfaces")
        .as_array()
        .into_iter()
        .flatten()
    {
        let ip = text(interface, "/networkIP");
        if !ip.is_empty() {
            internal.push(ip);
        }
        for access in at(interface, "/accessConfigs")
            .as_array()
            .into_iter()
            .flatten()
        {
            let ip = text(access, "/natIP");
            if !ip.is_empty() {
                external.push(ip);
            }
        }
    }
    (internal, external)
}

/// A VM as the pane and agents read it.
pub fn instance(value: &Value) -> Value {
    let (internal, external) = ips(value);
    let disks: Vec<Value> = at(value, "/disks")
        .as_array()
        .into_iter()
        .flatten()
        .map(|disk| {
            json!({
                "name": short(&text(disk, "/source")),
                "sizeGb": at(disk, "/diskSizeGb").as_str().and_then(|size| size.parse::<u64>().ok()).or_else(|| at(disk, "/diskSizeGb").as_u64()),
                "boot": at(disk, "/boot").as_bool() == Some(true),
                "mode": at(disk, "/mode"),
            })
        })
        .collect();
    let model = text(value, "/scheduling/provisioningModel");
    json!({
        "id": at(value, "/id"),
        "name": at(value, "/name"),
        "zone": short(&text(value, "/zone")),
        "machineType": short(&text(value, "/machineType")),
        "status": at(value, "/status"),
        "internalIps": internal,
        "externalIps": external,
        "disks": disks,
        "spot": model == "SPOT" || at(value, "/scheduling/preemptible").as_bool() == Some(true),
        "cpuPlatform": at(value, "/cpuPlatform"),
        "serviceAccount": at(value, "/serviceAccounts/0/email"),
        "tags": at(value, "/tags/items"),
        "labels": at(value, "/labels"),
        "createdAt": at(value, "/creationTimestamp"),
        "startedAt": at(value, "/lastStartTimestamp"),
        "stoppedAt": at(value, "/lastStopTimestamp"),
    })
}

/// The VMs in an aggregated listing's zones.
pub fn instances_of(page: &Value) -> Vec<Value> {
    at(page, "/items")
        .as_object()
        .into_iter()
        .flat_map(|zones| zones.values())
        .flat_map(|zone| {
            at(zone, "/instances")
                .as_array()
                .cloned()
                .unwrap_or_default()
        })
        .collect()
}

#[derive(Deserialize)]
pub struct Listing {
    #[serde(flatten)]
    pub target: Target,
    #[serde(default)]
    pub query: Option<String>,
}

/// Every VM in the project, in every zone, by name.
pub async fn instances(input: Listing) -> GcpResult<Vec<Value>> {
    let project = input.target.project()?;
    let url = format!("{API}/projects/{project}/aggregated/instances");
    let mut raw = Vec::new();
    let mut page_token: Option<String> = None;
    for _ in 0..MAX_PAGES {
        let mut query = vec![
            ("maxResults", "500".to_string()),
            ("returnPartialSuccess", "true".to_string()),
        ];
        if let Some(token) = &page_token {
            query.push(("pageToken", token.clone()));
        }
        let page = client::get(&input.target, &url, &query).await?;
        raw.extend(instances_of(&page));
        page_token = page
            .get("nextPageToken")
            .and_then(Value::as_str)
            .filter(|t| !t.is_empty())
            .map(str::to_string);
        if page_token.is_none() {
            break;
        }
    }
    let words = input.query.unwrap_or_default().trim().to_lowercase();
    let mut listed: Vec<Value> = raw
        .iter()
        .map(instance)
        .filter(|each| words.is_empty() || text(each, "/name").to_lowercase().contains(&words))
        .collect();
    listed.sort_by_key(|each| text(each, "/name"));
    Ok(listed)
}

#[derive(Deserialize)]
pub struct InstanceRef {
    #[serde(flatten)]
    pub target: Target,
    pub zone: String,
    pub instance: String,
}

fn instance_url(input: &InstanceRef) -> GcpResult<String> {
    let project = input.target.project()?;
    let zone = segment("zone", &input.zone)?;
    let name = segment("instance", &input.instance)?;
    Ok(format!(
        "{API}/projects/{project}/zones/{zone}/instances/{name}"
    ))
}

/// One VM, with the command that opens a shell on it.
pub async fn instance_detail(input: InstanceRef) -> GcpResult<Value> {
    let url = instance_url(&input)?;
    let raw = client::get(&input.target, &url, &[]).await?;
    let mut read = instance(&raw);
    if let Some(object) = read.as_object_mut() {
        object.insert("ssh".into(), json!(ssh_command(&input)?));
    }
    Ok(read)
}

/// `gcloud compute ssh`, which sets up keys and connects, as an argument list for a terminal.
pub fn ssh_command(input: &InstanceRef) -> GcpResult<Vec<String>> {
    let project = input.target.project()?;
    let mut command = vec![
        "gcloud".to_string(),
        "compute".to_string(),
        "ssh".to_string(),
        segment("instance", &input.instance)?,
        format!("--zone={}", segment("zone", &input.zone)?),
        format!("--project={project}"),
    ];
    if let Some(account) = input.target.account() {
        command.push(format!("--account={}", crate::gcloud::value(account)?));
    }
    Ok(command)
}

#[derive(Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "lowercase")]
pub enum Power {
    Start,
    Stop,
    Reset,
    Suspend,
    Resume,
}

impl Power {
    fn verb(self) -> &'static str {
        match self {
            Power::Start => "start",
            Power::Stop => "stop",
            Power::Reset => "reset",
            Power::Suspend => "suspend",
            Power::Resume => "resume",
        }
    }
}

#[derive(Deserialize)]
pub struct PowerChange {
    #[serde(flatten)]
    pub instance: InstanceRef,
    pub action: Power,
}

/// Starts, stops, resets, suspends or resumes a VM, answering with the operation Google started.
pub async fn power(input: PowerChange) -> GcpResult<Value> {
    let url = format!("{}/{}", instance_url(&input.instance)?, input.action.verb());
    let operation = client::post(&input.instance.target, &url, &json!({})).await?;
    Ok(operation_of(&operation))
}

pub fn operation_of(operation: &Value) -> Value {
    json!({
        "operation": at(operation, "/name"),
        "status": at(operation, "/status"),
        "error": at(operation, "/error/errors/0/message"),
    })
}

#[derive(Deserialize)]
pub struct Serial {
    #[serde(flatten)]
    pub instance: InstanceRef,
    /// Where to read from, as the last answer's `next` said; none reads the latest output.
    #[serde(default)]
    pub start: Option<i64>,
}

/// What a VM has printed to its serial console: boot messages, kernel output, startup scripts.
pub async fn serial(input: Serial) -> GcpResult<Value> {
    let url = format!("{}/serialPort", instance_url(&input.instance)?);
    let mut query = vec![("port", "1".to_string())];
    if let Some(start) = input.start {
        query.push(("start", start.to_string()));
    }
    let page = client::get(&input.instance.target, &url, &query).await?;
    let contents = text(&page, "/contents");
    let skipped = contents.len().saturating_sub(MAX_SERIAL_BYTES);
    let tail = contents
        .char_indices()
        .find(|(index, _)| *index >= skipped)
        .map(|(index, _)| contents.get(index..).unwrap_or_default())
        .unwrap_or_default();
    if page.is_null() {
        return Err(GcpError::Response(
            "Compute Engine sent no serial output".into(),
        ));
    }
    Ok(json!({
        "contents": tail,
        "start": at(&page, "/start"),
        "next": at(&page, "/next"),
        "trimmed": skipped > 0,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn raw() -> Value {
        json!({
            "id": "123", "name": "web-1", "status": "RUNNING",
            "zone": "https://www.googleapis.com/compute/v1/projects/acme/zones/us-central1-a",
            "machineType": "https://www.googleapis.com/compute/v1/projects/acme/zones/us-central1-a/machineTypes/e2-medium",
            "networkInterfaces": [{ "networkIP": "10.0.0.5", "accessConfigs": [{ "natIP": "34.1.2.3" }] }],
            "disks": [{ "source": "https://.../disks/web-1", "diskSizeGb": "20", "boot": true, "mode": "READ_WRITE" }],
            "scheduling": { "provisioningModel": "SPOT" },
            "metadata": { "items": [{ "key": "startup-script", "value": "export TOKEN=secret" }] }
        })
    }

    #[test]
    fn a_vm_reads_with_its_zone_type_addresses_and_disks() {
        let read = instance(&raw());
        assert_eq!(read["zone"], "us-central1-a");
        assert_eq!(read["machineType"], "e2-medium");
        assert_eq!(read["internalIps"], json!(["10.0.0.5"]));
        assert_eq!(read["externalIps"], json!(["34.1.2.3"]));
        assert_eq!(read["disks"][0]["sizeGb"], 20);
        assert_eq!(read["spot"], true);
        assert!(
            !read.to_string().contains("secret"),
            "metadata, which can hold startup scripts, is left out"
        );
    }

    #[test]
    fn an_aggregated_listing_yields_every_zones_vms() {
        let page = json!({ "items": {
            "zones/us-central1-a": { "instances": [raw()] },
            "zones/europe-west1-b": { "warning": { "code": "NO_RESULTS_ON_PAGE" } },
            "zones/asia-east1-a": { "instances": [{ "name": "db-1" }] }
        } });
        assert_eq!(instances_of(&page).len(), 2);
    }

    #[test]
    fn a_shell_opens_through_gcloud_compute_ssh() {
        let input = InstanceRef {
            target: Target {
                account: Some("ankit@example.com".into()),
                project: Some("acme".into()),
            },
            zone: "us-central1-a".into(),
            instance: "web-1".into(),
        };
        assert_eq!(
            ssh_command(&input).expect("a command"),
            [
                "gcloud",
                "compute",
                "ssh",
                "web-1",
                "--zone=us-central1-a",
                "--project=acme",
                "--account=ankit@example.com"
            ]
        );
        let sneaky = InstanceRef {
            instance: "web-1;rm".into(),
            ..input
        };
        assert!(ssh_command(&sneaky).is_err());
    }
}
