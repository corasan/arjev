use std::collections::BTreeMap;
use std::thread::sleep;
use std::time::Duration;

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

pub const ENDPOINT: &str = "https://openrouter.ai/api/alpha/decisions";
pub const DEFAULT_MODEL: &str = "typesafe/jev-1.13";
const ATTEMPTS: u32 = 3;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct NoulCriteria {
    #[serde(rename = "true")]
    pub yes: String,
    #[serde(rename = "false")]
    pub no: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Question {
    Noul {
        instructions: String,
        criteria: NoulCriteria,
    },
    Choice {
        instructions: String,
        criteria: BTreeMap<String, String>,
    },
    Score {
        instructions: String,
        criteria: Vec<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct ChoiceAnswer {
    pub choice: String,
    pub probabilities: BTreeMap<String, f64>,
    pub confidence: f64,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Answer {
    Noul {
        noul: f64,
    },
    Choice(ChoiceAnswer),
    Score {
        score: f64,
        legend: BTreeMap<String, String>,
        probabilities: BTreeMap<String, f64>,
        confidence: f64,
    },
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Usage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    #[serde(default)]
    pub cost: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Decision {
    pub model: String,
    pub answers: BTreeMap<String, Answer>,
    pub usage: Usage,
    #[serde(default)]
    pub provider: Option<String>,
}

impl Decision {
    pub fn answer(&self, name: &str) -> Result<&Answer> {
        self.answers
            .get(name)
            .with_context(|| format!("Jev returned no answer named `{name}`"))
    }
}

pub struct JevClient {
    api_key: String,
    model: String,
    agent: ureq::Agent,
}

impl JevClient {
    pub fn from_env() -> Result<Self> {
        let api_key = std::env::var("OPENROUTER_API_KEY").map_err(|_| {
            anyhow::anyhow!("OPENROUTER_API_KEY is not set; export an OpenRouter API key to ask Jev")
        })?;
        let model = std::env::var("ARJEV_MODEL").unwrap_or_else(|_| DEFAULT_MODEL.to_string());
        let config = ureq::Agent::config_builder()
            .http_status_as_error(false)
            .timeout_global(Some(Duration::from_secs(120)))
            .build();
        Ok(Self {
            api_key,
            model,
            agent: config.into(),
        })
    }

    pub fn decide(
        &self,
        state: &Value,
        questions: &BTreeMap<String, Question>,
    ) -> Result<Decision> {
        let body = request_body(&self.model, state, questions);
        let mut backoff = Duration::from_millis(500);
        for attempt in 1..=ATTEMPTS {
            let mut response = self
                .agent
                .post(ENDPOINT)
                .header("Authorization", format!("Bearer {}", self.api_key))
                .header("Content-Type", "application/json")
                .send_json(&body)
                .context("Jev could not be reached at OpenRouter")?;

            let status = response.status().as_u16();
            if (status == 429 || status == 529) && attempt < ATTEMPTS {
                sleep(backoff);
                backoff *= 2;
                continue;
            }
            let text = response.body_mut().read_to_string()?;
            if !(200..300).contains(&status) {
                bail!("Jev answered with status {status}: {text}");
            }
            return serde_json::from_str(&text)
                .with_context(|| format!("Jev returned an unexpected body: {text}"));
        }
        bail!("Jev stayed rate limited after {ATTEMPTS} attempts")
    }
}

pub fn request_body(
    model: &str,
    state: &Value,
    questions: &BTreeMap<String, Question>,
) -> Value {
    json!({ "model": model, "state": state, "questions": questions })
}
