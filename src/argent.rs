use std::cmp::Reverse;
use std::fs;
use std::path::PathBuf;
use std::time::Duration;

use anyhow::{anyhow, bail, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Device {
    pub platform: String,
    #[serde(alias = "serial", alias = "id")]
    pub udid: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub state: String,
}

impl Device {
    pub fn is_running(&self) -> bool {
        matches!(self.state.as_str(), "Booted" | "device" | "running" | "Running" | "connected")
    }
}

#[derive(Debug, Deserialize)]
struct DeviceList {
    devices: Vec<Device>,
}

#[derive(Debug, Deserialize)]
pub struct Tool {
    pub name: String,
    #[serde(default)]
    pub description: String,
}

#[derive(Debug, Deserialize)]
struct ToolList {
    tools: Vec<Tool>,
}

#[derive(Debug, Deserialize)]
struct Described {
    description: String,
}

#[derive(Debug, Deserialize)]
struct Endpoint {
    port: u16,
    host: String,
    token: String,
    #[serde(default)]
    version: String,
}

pub struct ArgentClient {
    base_url: String,
    token: String,
    agent: ureq::Agent,
}

impl ArgentClient {
    pub fn discover() -> Result<Self> {
        if let (Ok(base_url), Ok(token)) = (
            std::env::var("ARGENT_URL"),
            std::env::var("ARGENT_TOKEN"),
        ) {
            return Ok(Self::new(base_url.trim_end_matches('/').to_string(), token));
        }

        let mut endpoints = read_endpoints()?;
        if endpoints.is_empty() {
            bail!("no Argent tool-server files found in ~/.argent; start Argent or set ARGENT_URL and ARGENT_TOKEN");
        }
        endpoints.sort_by_key(|endpoint| Reverse(version_key(&endpoint.version)));

        for endpoint in &endpoints {
            let client = Self::new(
                format!("http://{}:{}", endpoint.host, endpoint.port),
                endpoint.token.clone(),
            );
            if client.list_tools().is_ok() {
                return Ok(client);
            }
        }
        bail!("found {} Argent tool-server files but none answered; start Argent or set ARGENT_URL and ARGENT_TOKEN", endpoints.len())
    }

    fn new(base_url: String, token: String) -> Self {
        let config = ureq::Agent::config_builder()
            .http_status_as_error(false)
            .timeout_global(Some(Duration::from_secs(120)))
            .build();
        Self {
            base_url,
            token,
            agent: config.into(),
        }
    }

    pub fn call(&self, tool: &str, args: Value) -> Result<Value> {
        let url = format!("{}/tools/{}", self.base_url, tool);
        let mut response = self
            .agent
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.token))
            .header("Content-Type", "application/json")
            .send_json(&args)
            .with_context(|| format!("argent tool `{tool}` could not be reached at {url}"))?;

        let status = response.status().as_u16();
        let body: Value = response
            .body_mut()
            .read_json()
            .with_context(|| format!("argent tool `{tool}` returned a body that is not JSON"))?;

        if !(200..300).contains(&status) {
            let reason = body
                .get("error")
                .or_else(|| body.get("message"))
                .and_then(Value::as_str)
                .unwrap_or("no reason given");
            bail!("argent tool `{tool}` failed with status {status}: {reason}");
        }
        body.get("data")
            .cloned()
            .ok_or_else(|| anyhow!("argent tool `{tool}` returned no data field"))
    }

    pub fn list_tools(&self) -> Result<Vec<Tool>> {
        let url = format!("{}/tools", self.base_url);
        let mut response = self
            .agent
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.token))
            .call()
            .with_context(|| format!("argent tool-server at {url} could not be reached"))?;
        let status = response.status().as_u16();
        if !(200..300).contains(&status) {
            bail!("argent tool-server at {url} answered with status {status}");
        }
        let list: ToolList = response.body_mut().read_json()?;
        Ok(list.tools)
    }

    pub fn list_devices(&self) -> Result<Vec<Device>> {
        let data = self.call("list-devices", json!({}))?;
        let list: DeviceList = serde_json::from_value(data)?;
        Ok(list.devices)
    }

    pub fn describe(&self, udid: &str) -> Result<String> {
        let data = self.call("describe", json!({ "udid": udid }))?;
        let described: Described = serde_json::from_value(data)?;
        Ok(described.description)
    }
}

fn read_endpoints() -> Result<Vec<Endpoint>> {
    let home = std::env::var("HOME").context("HOME is not set")?;
    let dir = PathBuf::from(home).join(".argent");
    let Ok(entries) = fs::read_dir(&dir) else {
        return Ok(Vec::new());
    };
    let mut endpoints = Vec::new();
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if !name.starts_with("tool-server-") || !name.ends_with(".json") {
            continue;
        }
        let Ok(text) = fs::read_to_string(entry.path()) else {
            continue;
        };
        if let Ok(endpoint) = serde_json::from_str::<Endpoint>(&text) {
            endpoints.push(endpoint);
        }
    }
    Ok(endpoints)
}

fn version_key(version: &str) -> (u32, u32, u32) {
    let mut parts = version
        .split(['.', '-', '+'])
        .map(|part| part.parse::<u32>().unwrap_or(0));
    (
        parts.next().unwrap_or(0),
        parts.next().unwrap_or(0),
        parts.next().unwrap_or(0),
    )
}
