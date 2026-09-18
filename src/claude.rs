use std::collections::BTreeMap;
use std::io::Write;
use std::process::{Command, Stdio};

use anyhow::{bail, Context, Result};
use serde_json::{json, Map, Value};

use crate::jev::{Answer, Decision, Question, Usage};

pub const DEFAULT_MODEL: &str = "claude-opus-5";
pub const BINARY: &str = "claude";
const INSTRUCTIONS: &str = "You are a decision function. Reply only with the JSON object of answers. Probabilities must sum to 1. A noul is the probability that the answer to the question is yes.";

pub struct ClaudeClient {
    model: String,
}

impl ClaudeClient {
    pub fn new(model: String) -> Self {
        Self { model }
    }

    pub fn model(&self) -> &str {
        &self.model
    }

    pub fn decide(
        &self,
        state: &Value,
        questions: &BTreeMap<String, Question>,
    ) -> Result<Decision> {
        let mut child = Command::new(BINARY)
            .args(arguments(&self.model, questions))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .with_context(|| format!("`{BINARY}` could not be started; install Claude Code and log in"))?;
        child
            .stdin
            .take()
            .context("claude stdin is closed")?
            .write_all(prompt(state, questions).as_bytes())?;
        let output = child.wait_with_output()?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            bail!("`{BINARY}` exited with {}: {stderr}{stdout}", output.status);
        }
        decision(&self.model, &stdout)
    }
}

pub fn arguments(model: &str, questions: &BTreeMap<String, Question>) -> Vec<String> {
    [
        "-p",
        "--model",
        model,
        "--output-format",
        "json",
        "--json-schema",
        &answer_schema(questions).to_string(),
        "--tools",
        "",
        "--max-turns",
        "3",
        "--system-prompt",
        INSTRUCTIONS,
        "--strict-mcp-config",
        "--mcp-config",
        "{\"mcpServers\":{}}",
    ]
    .into_iter()
    .map(str::to_string)
    .collect()
}

pub fn prompt(state: &Value, questions: &BTreeMap<String, Question>) -> String {
    json!({ "state": state, "questions": questions }).to_string()
}

pub fn decision(model: &str, text: &str) -> Result<Decision> {
    let body: Value = serde_json::from_str(text.trim())
        .with_context(|| format!("claude returned an unexpected body: {text}"))?;
    let result = match &body {
        Value::Array(events) => events.last().cloned().unwrap_or(Value::Null),
        other => other.clone(),
    };
    if result.get("is_error").and_then(Value::as_bool) == Some(true) {
        bail!(
            "claude reported an error: {}",
            result.get("result").and_then(Value::as_str).unwrap_or("no detail")
        );
    }
    let answers = result
        .get("structured_output")
        .with_context(|| format!("claude returned no structured_output: {result}"))?;
    let answers: BTreeMap<String, Answer> = serde_json::from_value(answers.clone())
        .with_context(|| format!("claude returned answers it cannot keep to: {answers}"))?;
    let usage = result
        .get("modelUsage")
        .and_then(Value::as_object)
        .and_then(|models| models.values().next())
        .map(usage)
        .unwrap_or_default();
    Ok(Decision {
        model: model.to_string(),
        answers,
        usage,
        provider: Some("claude-cli".to_string()),
    })
}

fn usage(model_usage: &Value) -> Usage {
    let count = |key: &str| model_usage.get(key).and_then(Value::as_u64).unwrap_or(0);
    Usage {
        input_tokens: count("inputTokens") + count("cacheCreationInputTokens") + count("cacheReadInputTokens"),
        output_tokens: count("outputTokens"),
        cost: model_usage.get("costUSD").and_then(Value::as_f64),
    }
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
