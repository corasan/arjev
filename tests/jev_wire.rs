use std::collections::BTreeMap;

use arjev::jev::{request_body, Answer, Decision, NoulCriteria, Question};
use serde_json::json;

#[test]
fn the_request_body_matches_the_documented_question_shapes() {
    let questions = BTreeMap::from([
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
        (
            "legibility".to_string(),
            Question::Score {
                instructions: "How legible is the screen?".to_string(),
                criteria: vec!["unreadable".to_string(), "clear".to_string()],
            },
        ),
    ]);

    let body = request_body("typesafe/jev-1.13", &json!("AXButton \"General\""), &questions);

    assert_eq!(
        body,
        json!({
            "model": "typesafe/jev-1.13",
            "state": "AXButton \"General\"",
            "questions": {
                "legibility": {
                    "type": "score",
                    "instructions": "How legible is the screen?",
                    "criteria": ["unreadable", "clear"]
                },
                "target": {
                    "type": "choice",
                    "instructions": "Which element opens General?",
                    "criteria": { "e0": "AXButton \"General\"", "e1": "AXButton \"Camera\"" }
                },
                "visible": {
                    "type": "noul",
                    "instructions": "Is the Settings root list visible?",
                    "criteria": { "true": "The root list is visible.", "false": "Something else is on screen." }
                }
            }
        })
    );
}

#[test]
fn the_documented_response_parses_into_typed_answers() {
    let response = json!({
        "model": "typesafe/jev-1.13",
        "answers": {
            "visible": { "type": "noul", "noul": 0.93 },
            "target": {
                "type": "choice",
                "choice": "e0",
                "probabilities": { "e0": 0.81, "e1": 0.19 },
                "confidence": 0.74
            },
            "legibility": {
                "type": "score",
                "score": 1.0,
                "legend": { "0": "unreadable", "1": "clear" },
                "probabilities": { "0": 0.05, "1": 0.95 },
                "confidence": 0.9
            }
        },
        "usage": { "input_tokens": 1200, "output_tokens": 8, "cost": 0.0004 },
        "provider": "typesafe"
    });

    let decision: Decision = serde_json::from_value(response).expect("documented response parses");

    assert_eq!(decision.model, "typesafe/jev-1.13");
    assert_eq!(decision.usage.input_tokens, 1200);
    assert_eq!(decision.usage.cost, Some(0.0004));
    assert_eq!(decision.provider.as_deref(), Some("typesafe"));

    let Answer::Noul { noul } = decision.answer("visible").unwrap() else {
        panic!("visible is a noul answer");
    };
    assert_eq!(*noul, 0.93);

    let Answer::Choice(chosen) = decision.answer("target").unwrap() else {
        panic!("target is a choice answer");
    };
    assert_eq!(chosen.choice, "e0");
    assert_eq!(chosen.probabilities["e0"], 0.81);
    assert_eq!(chosen.confidence, 0.74);

    let Answer::Score { score, legend, .. } = decision.answer("legibility").unwrap() else {
        panic!("legibility is a score answer");
    };
    assert_eq!(*score, 1.0);
    assert_eq!(legend["1"], "clear");
}

#[test]
fn usage_parses_without_the_optional_cost_and_provider() {
    let decision: Decision = serde_json::from_value(json!({
        "model": "typesafe/jev-1.13",
        "answers": { "visible": { "type": "noul", "noul": 0.1 } },
        "usage": { "input_tokens": 10, "output_tokens": 1 }
    }))
    .expect("optional fields may be absent");

    assert_eq!(decision.usage.cost, None);
    assert_eq!(decision.provider, None);
}
