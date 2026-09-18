use std::collections::BTreeMap;

use arjev::chat::{decision, request_body};
use arjev::jev::{Answer, NoulCriteria, Question};
use serde_json::{json, Value};

fn questions() -> BTreeMap<String, Question> {
    BTreeMap::from([
        (
            "visible".to_string(),
            Question::Noul {
                instructions: "Is the Settings root list visible?".to_string(),
                criteria: NoulCriteria {
                    yes: "The root list is visible.".to_string(),
                    no: "Something else is on screen.".to_string(),
                },
            },
        ),
        (
            "target".to_string(),
            Question::Choice {
                instructions: "Which element opens General?".to_string(),
                criteria: BTreeMap::from([
                    ("e0".to_string(), "AXButton \"General\"".to_string()),
                    ("e1".to_string(), "AXButton \"Camera\"".to_string()),
                ]),
            },
        ),
    ])
}

#[test]
fn the_response_schema_pins_one_shape_per_question() {
    let body = request_body("anthropic/claude-opus-5", &json!("a screen"), &questions());

    assert_eq!(
        body["response_format"],
        json!({
            "type": "json_schema",
            "json_schema": {
                "name": "answers",
                "strict": true,
                "schema": {
                    "type": "object",
                    "additionalProperties": false,
                    "required": ["target", "visible"],
                    "properties": {
                        "target": {
                            "type": "object",
                            "additionalProperties": false,
                            "required": ["choice", "confidence", "probabilities", "type"],
                            "properties": {
                                "type": { "const": "choice" },
                                "choice": { "enum": ["e0", "e1"] },
                                "confidence": { "type": "number" },
                                "probabilities": {
                                    "type": "object",
                                    "additionalProperties": false,
                                    "required": ["e0", "e1"],
                                    "properties": {
                                        "e0": { "type": "number" },
                                        "e1": { "type": "number" }
                                    }
                                }
                            }
                        },
                        "visible": {
                            "type": "object",
                            "additionalProperties": false,
                            "required": ["noul", "type"],
                            "properties": {
                                "type": { "const": "noul" },
                                "noul": { "type": "number" }
                            }
                        }
                    }
                }
            }
        })
    );
}

#[test]
fn the_state_and_questions_travel_as_one_user_message() {
    let body = request_body("anthropic/claude-opus-5", &json!("a screen"), &questions());

    assert_eq!(body["model"], "anthropic/claude-opus-5");
    assert_eq!(body["temperature"], 0);
    assert_eq!(body["usage"], json!({ "include": true }));
    assert_eq!(body["messages"][0]["role"], "system");
    assert_eq!(body["messages"][1]["role"], "user");

    let instructions = body["messages"][0]["content"].as_str().unwrap();
    assert!(instructions.contains("Probabilities must sum to 1"));
    assert!(instructions.contains("noul is the probability"));

    let carried: Value = serde_json::from_str(body["messages"][1]["content"].as_str().unwrap())
        .expect("the user message is a JSON string");
    assert_eq!(carried["state"], "a screen");
    assert_eq!(carried["questions"]["visible"]["type"], "noul");
    assert_eq!(
        carried["questions"]["visible"]["criteria"]["true"],
        "The root list is visible."
    );
    assert_eq!(
        carried["questions"]["target"]["criteria"]["e0"],
        "AXButton \"General\""
    );
}

#[test]
fn a_chat_completion_becomes_the_same_decision_shape_as_jev() {
    let content = json!({
        "visible": { "type": "noul", "noul": 0.98 },
        "target": {
            "type": "choice",
            "choice": "e0",
            "probabilities": { "e0": 0.97, "e1": 0.03 },
            "confidence": 0.95
        }
    })
    .to_string();
    let completion = json!({
        "model": "anthropic/claude-opus-5",
        "provider": "Anthropic",
        "choices": [{ "message": { "role": "assistant", "content": content } }],
        "usage": { "prompt_tokens": 1820, "completion_tokens": 96, "cost": 0.0271 }
    })
    .to_string();

    let decision = decision("fallback/model", &completion).expect("the completion parses");

    assert_eq!(decision.model, "anthropic/claude-opus-5");
    assert_eq!(decision.provider.as_deref(), Some("Anthropic"));
    assert_eq!(decision.usage.input_tokens, 1820);
    assert_eq!(decision.usage.output_tokens, 96);
    assert_eq!(decision.usage.cost, Some(0.0271));

    let Answer::Noul { noul } = decision.answer("visible").unwrap() else {
        panic!("visible is a noul answer");
    };
    assert_eq!(*noul, 0.98);

    let Answer::Choice(chosen) = decision.answer("target").unwrap() else {
        panic!("target is a choice answer");
    };
    assert_eq!(chosen.choice, "e0");
    assert_eq!(chosen.probabilities["e0"], 0.97);
    assert_eq!(chosen.confidence, 0.95);
}

#[test]
fn a_completion_whose_content_is_not_the_answer_map_is_an_error() {
    let completion = json!({
        "model": "anthropic/claude-opus-5",
        "choices": [{ "message": { "content": "I think the answer is probably yes." } }],
        "usage": { "prompt_tokens": 10, "completion_tokens": 9 }
    })
    .to_string();

    let error = decision("fallback/model", &completion).expect_err("prose is not an answer map");
    assert!(error.to_string().contains("answers it cannot keep to"));
}
