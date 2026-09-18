use std::path::Path;

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;

const MAX_PREREQUISITE_DEPTH: u32 = 8;

pub fn default_threshold() -> f64 {
    0.8
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Plan {
    pub name: String,
    pub device: DeviceSelector,
    #[serde(default)]
    pub prerequisite: Option<String>,
    pub steps: Vec<Step>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DeviceSelector {
    Udid(String),
    Name(String),
    First,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Step {
    Act {
        tool: String,
        #[serde(default)]
        args: Value,
    },
    Assert {
        name: String,
        question: String,
        #[serde(default = "default_threshold")]
        threshold: f64,
    },
    Choose {
        name: String,
        question: String,
        then: ChooseAction,
        #[serde(default)]
        roles: Vec<String>,
    },
}

impl Step {
    pub fn label(&self) -> String {
        match self {
            Step::Act { tool, .. } => format!("act {tool}"),
            Step::Assert { name, .. } => format!("assert {name}"),
            Step::Choose { name, .. } => format!("choose {name}"),
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ChooseAction {
    Tap,
}

impl Plan {
    pub fn load(path: &Path) -> Result<Self> {
        Self::load_at(path, 0)
    }

    fn load_at(path: &Path, depth: u32) -> Result<Self> {
        if depth > MAX_PREREQUISITE_DEPTH {
            bail!("plan prerequisites nest deeper than {MAX_PREREQUISITE_DEPTH} levels at {}", path.display());
        }
        let text = std::fs::read_to_string(path)
            .with_context(|| format!("plan {} could not be read", path.display()))?;
        let mut plan: Plan = serde_yaml::from_str(&text)
            .with_context(|| format!("plan {} is not valid arjev YAML", path.display()))?;

        if let Some(prerequisite) = plan.prerequisite.take() {
            let base = path.parent().unwrap_or_else(|| Path::new("."));
            let earlier = Self::load_at(&base.join(prerequisite), depth + 1)?;
            plan.steps.splice(0..0, earlier.steps);
        }
        Ok(plan)
    }
}
