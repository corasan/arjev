use std::collections::BTreeMap;
use std::thread::sleep;
use std::time::Duration;

use anyhow::{bail, Context, Result};
use serde_json::{json, Map, Value};

use crate::jev::{Answer, Decision, Question, Usage};

pub const ENDPOINT: &str = "https://openrouter.ai/api/v1/chat/completions";
pub const DEFAULT_MODEL: &str = "anthropic/claude-opus-5";
const ATTEMPTS: u32 = 3;
const INSTRUCTIONS: &str = "You are a decision function. Reply only with the JSON object of answers. Probabilities must sum to 1. A noul is the probability that the answer to the question is yes.";

pub struct ChatClient {
    api_key: String,
    model: String,
    agent: ureq::Agent,
}

impl ChatClient {
    pub fn new(api_key: String, model: String) -> Self {
        let config = ureq::Agent::config_builder()
            .http_status_as_error(false)
            .timeout_global(Some(Duration::from_secs(300)))
            .build();
        Self {
            api_key,
            model,
            agent: config.into(),
        }
    }

    pub fn model(&self) -> &str {
        &self.model
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
                .context("the chat model could not be reached at OpenRouter")?;

            let status = response.status().as_u16();
            if (status == 429 || status == 529) && attempt < ATTEMPTS {
                sleep(backoff);
                backoff *= 2;
                continue;
            }
            let text = response.body_mut().read_to_string()?;
            if !(200..300).contains(&status) {
                bail!("the chat model answered with status {status}: {text}");
            }
            return decision(&self.model, &text);
        }
        bail!("the chat model stayed rate limited after {ATTEMPTS} attempts")
    }
}

pub fn decision(model: &str, text: &str) -> Result<Decision> {
    let body: Value = serde_json::from_str(text)
        .with_context(|| format!("the chat model returned an unexpected body: {text}"))?;
    let content = body
        .pointer("/choices/0/message/content")
        .and_then(Value::as_str)
        .with_context(|| format!("the chat model returned no message content: {text}"))?;
    let usage = match body.get("usage") {
        Some(usage) => serde_json::from_value(usage.clone())
            .with_context(|| format!("the chat model returned unreadable usage: {usage}"))?,
        None => Usage::default(),
    };
    Ok(Decision {
        model: body
            .get("model")
            .and_then(Value::as_str)
            .unwrap_or(model)
            .to_string(),
        answers: answers(content)?,
        usage,
        provider: body
            .get("provider")
            .and_then(Value::as_str)
            .map(str::to_string),
    })
}

pub fn answers(content: &str) -> Result<BTreeMap<String, Answer>> {
    serde_json::from_str(content)
        .with_context(|| format!("the chat model returned answers it cannot keep to: {content}"))
}

pub fn request_body(
    model: &str,
    state: &Value,
    questions: &BTreeMap<String, Question>,
) -> Value {
    json!({
        "model": model,
        "messages": [
            { "role": "system", "content": INSTRUCTIONS },
            { "role": "user", "content": json!({ "state": state, "questions": questions }).to_string() }
        ],
        "temperature": 0,
        "response_format": {
            "type": "json_schema",
            "json_schema": { "name": "answers", "strict": true, "schema": answer_schema(questions) }
        },
        "usage": { "include": true }
    })
}

pub fn answer_schema(questions: &BTreeMap<String, Question>) -> Value {
    let properties: Map<String, Value> = questions
        .iter()
        .map(|(name, question)| (name.clone(), question_schema(question)))
        .collect();
    object_schema(properties)
}

fn question_schema(question: &Question) -> Value {
    match question {
        Question::Noul { .. } => object_schema(
            [
                ("type".to_string(), json!({ "const": "noul" })),
                ("noul".to_string(), json!({ "type": "number" })),
            ]
            .into_iter()
            .collect(),
        ),
        Question::Choice { criteria, .. } => {
            let ids: Vec<&String> = criteria.keys().collect();
            object_schema(
                [
                    ("type".to_string(), json!({ "const": "choice" })),
                    ("choice".to_string(), json!({ "enum": ids })),
                    (
                        "probabilities".to_string(),
                        keyed_schema(&ids, json!({ "type": "number" })),
                    ),
                    ("confidence".to_string(), json!({ "type": "number" })),
                ]
                .into_iter()
                .collect(),
            )
        }
        Question::Score { criteria, .. } => {
            let levels: Vec<String> = (0..criteria.len()).map(|level| level.to_string()).collect();
            let ids: Vec<&String> = levels.iter().collect();
            object_schema(
                [
                    ("type".to_string(), json!({ "const": "score" })),
                    ("score".to_string(), json!({ "type": "number" })),
                    (
                        "legend".to_string(),
                        keyed_schema(&ids, json!({ "type": "string" })),
                    ),
                    (
                        "probabilities".to_string(),
                        keyed_schema(&ids, json!({ "type": "number" })),
                    ),
                    ("confidence".to_string(), json!({ "type": "number" })),
                ]
                .into_iter()
                .collect(),
            )
        }
    }
}

fn keyed_schema(ids: &[&String], value: Value) -> Value {
    let properties = ids
        .iter()
        .map(|id| ((*id).clone(), value.clone()))
        .collect();
    object_schema(properties)
}

fn object_schema(properties: Map<String, Value>) -> Value {
    let required: Vec<&String> = properties.keys().collect();
    json!({
        "type": "object",
        "properties": properties,
        "required": required,
        "additionalProperties": false
    })
}
