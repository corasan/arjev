use std::collections::BTreeMap;

use arjev::claude::{answer_schema, arguments, decision, prompt};
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
    assert_eq!(
        answer_schema(&questions()),
        json!({
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
        })
    );
}

#[test]
fn the_cli_gets_the_schema_and_the_prompt_gets_the_state() {
    let args = arguments("claude-opus-5", &questions());

    assert_eq!(&args[..3], ["-p", "--model", "claude-opus-5"]);
    let schema_flag = args.iter().position(|arg| arg == "--json-schema").unwrap();
    let schema: Value = serde_json::from_str(&args[schema_flag + 1]).unwrap();
    assert_eq!(schema, answer_schema(&questions()));
    assert!(args.contains(&"--strict-mcp-config".to_string()));
    let system = args.iter().position(|arg| arg == "--system-prompt").unwrap();
    assert!(args[system + 1].contains("noul is the probability"));

    let carried: Value = serde_json::from_str(&prompt(&json!("a screen"), &questions())).unwrap();
    assert_eq!(carried["state"], "a screen");
    assert_eq!(carried["questions"]["visible"]["criteria"]["true"], "The root list is visible.");
    assert_eq!(carried["questions"]["target"]["criteria"]["e0"], "AXButton \"General\"");
}

#[test]
fn a_claude_result_becomes_the_same_decision_shape_as_jev() {
    let result = json!({
        "type": "result",
        "is_error": false,
        "result": "{\"visible\":{\"type\":\"noul\",\"noul\":0.98}}",
        "structured_output": {
            "visible": { "type": "noul", "noul": 0.98 },
            "target": {
                "type": "choice",
                "choice": "e0",
                "probabilities": { "e0": 0.97, "e1": 0.03 },
                "confidence": 0.95
            }
        },
        "total_cost_usd": 0.112235,
        "duration_ms": 1825,
        "modelUsage": {
            "claude-opus-5": {
                "inputTokens": 2,
                "outputTokens": 65,
                "cacheReadInputTokens": 0,
                "cacheCreationInputTokens": 11060,
                "costUSD": 0.112235
            }
        }
    });
    let stream = json!([{ "type": "system", "subtype": "init" }, result.clone()]).to_string();

    for text in [result.to_string(), stream] {
        let decision = decision("claude-opus-5", &text).expect("the result parses");

        assert_eq!(decision.model, "claude-opus-5");
        assert_eq!(decision.usage.input_tokens, 11062);
        assert_eq!(decision.usage.output_tokens, 65);
        assert_eq!(decision.usage.cost, Some(0.112235));

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
}

#[test]
fn a_claude_error_result_is_an_error() {
    let text = json!({ "type": "result", "is_error": true, "result": "Not logged in · Please run /login" }).to_string();

    let error = decision("claude-opus-5", &text).expect_err("an error result is not a decision");
    assert!(error.to_string().contains("Not logged in"));
}
