use std::collections::BTreeMap;

use anyhow::{anyhow, Result};
use clap::ValueEnum;
use serde_json::Value;

use crate::chat::{self, ChatClient};
use crate::jev::{self, Decision, JevClient, Question};

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
#[clap(rename_all = "lowercase")]
pub enum DeciderKind {
    Jev,
    Chat,
}

impl DeciderKind {
    fn parse(name: &str) -> Option<Self> {
        match name.trim().to_ascii_lowercase().as_str() {
            "jev" => Some(Self::Jev),
            "chat" => Some(Self::Chat),
            _ => None,
        }
    }

    fn default_model(self) -> &'static str {
        match self {
            Self::Jev => jev::DEFAULT_MODEL,
            Self::Chat => chat::DEFAULT_MODEL,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Selection {
    pub kind: DeciderKind,
    pub model: String,
}

impl Selection {
    pub fn resolve(kind: Option<DeciderKind>, model: Option<String>) -> Self {
        let kind = kind
            .or_else(|| std::env::var("ARJEV_DECIDER").ok().and_then(|name| DeciderKind::parse(&name)))
            .unwrap_or(DeciderKind::Jev);
        let model = model
            .or_else(|| std::env::var("ARJEV_MODEL").ok())
            .unwrap_or_else(|| kind.default_model().to_string());
        Self { kind, model }
    }
}

pub enum Decider {
    Jev(JevClient),
    Chat(ChatClient),
}

impl Decider {
    pub fn new(selection: &Selection) -> Result<Self> {
        let api_key = std::env::var("OPENROUTER_API_KEY").map_err(|_| {
            anyhow!("OPENROUTER_API_KEY is not set; export an OpenRouter API key to ask the decider")
        })?;
        let model = selection.model.clone();
        Ok(match selection.kind {
            DeciderKind::Jev => Self::Jev(JevClient::new(api_key, model)),
            DeciderKind::Chat => Self::Chat(ChatClient::new(api_key, model)),
        })
    }

    pub fn decide(
        &self,
        state: &Value,
        questions: &BTreeMap<String, Question>,
    ) -> Result<Decision> {
        match self {
            Self::Jev(client) => client.decide(state, questions),
            Self::Chat(client) => client.decide(state, questions),
        }
    }

    pub fn model(&self) -> &str {
        match self {
            Self::Jev(client) => client.model(),
            Self::Chat(client) => client.model(),
        }
    }
}

pub fn model_slug(model: &str) -> String {
    model
        .chars()
        .map(|letter| if letter.is_ascii_alphanumeric() { letter } else { '-' })
        .collect()
}
