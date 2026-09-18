use std::collections::BTreeMap;
use std::time::Instant;

use anyhow::{anyhow, bail, Result};
use serde::Serialize;
use serde_json::{json, Map, Value};

use crate::argent::{ArgentClient, Device};
use crate::decider::{Decider, Selection};
use crate::jev::{Answer, ChoiceAnswer, NoulCriteria, Question};
use crate::plan::{ChooseAction, DeviceSelector, Plan, Step};
use crate::screen::{Element, Screen};
use crate::verdict::{assert_verdict, choose_target, AssertVerdict, ChooseVerdict};

const MAX_CHOICE_OPTIONS: usize = 255;

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum Outcome {
    Pass,
    Fail { reason: String },
}

#[derive(Debug, Clone, Serialize)]
pub struct AnswerSummary {
    pub probability: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub confidence: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub choice: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct Spend {
    pub decide_ms: u128,
    pub input_tokens: u64,
    pub output_tokens: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cost: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct StepResult {
    pub step: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub answer: Option<AnswerSummary>,
    #[serde(flatten)]
    pub spend: Option<Spend>,
    pub elapsed_ms: u128,
    pub passed: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Report {
    pub plan: String,
    pub model: String,
    pub steps: Vec<StepResult>,
    pub total_ms: u128,
    pub outcome: Outcome,
}

#[derive(Default)]
struct Observed {
    answer: Option<AnswerSummary>,
    spend: Option<Spend>,
}

impl StepResult {
    pub fn line(&self) -> String {
        let mark = if self.passed { '\u{2713}' } else { '\u{2717}' };
        let probability = match &self.answer {
            Some(answer) => format!(" p={:.2}", answer.probability),
            None => String::new(),
        };
        let reason = match &self.reason {
            Some(reason) => format!("  {reason}"),
            None => String::new(),
        };
        format!(
            "{mark} {:<38}{:>7}ms{probability}{reason}",
            self.step, self.elapsed_ms
        )
    }
}

impl Report {
    pub fn passed(&self) -> bool {
        self.outcome == Outcome::Pass
    }
}

enum Inquiry<'a> {
    Assert {
        name: &'a str,
        question: &'a str,
        threshold: f64,
    },
    Choose {
        name: &'a str,
        question: &'a str,
        then: ChooseAction,
        roles: &'a [String],
        threshold: f64,
    },
}

enum Target {
    Tool { tool: String, args: Value },
    Element { x: f64, y: f64 },
}

enum Phase<'a> {
    Observe(Inquiry<'a>),
    Decide(Inquiry<'a>, Screen),
    Act(Target),
    Done(Outcome),
}

pub struct Runner {
    argent: ArgentClient,
    decider: Result<Decider, String>,
    model: String,
    udid: String,
    plan: Plan,
}

impl Runner {
    pub fn new(argent: ArgentClient, plan: Plan, udid: String, selection: &Selection) -> Self {
        Self {
            argent,
            decider: Decider::new(selection).map_err(|error| error.to_string()),
            model: selection.model.clone(),
            udid,
            plan,
        }
    }

    pub fn run(&self, mut observe: impl FnMut(&StepResult)) -> Report {
        let whole = Instant::now();
        let mut steps = Vec::new();
        let mut outcome = Outcome::Pass;

        for step in &self.plan.steps {
            let started = Instant::now();
            let mut observed = Observed::default();
            let step_outcome = match self.drive(step, &mut observed) {
                Ok(step_outcome) => step_outcome,
                Err(error) => Outcome::Fail {
                    reason: format!("{error:#}"),
                },
            };
            let reason = match &step_outcome {
                Outcome::Pass => None,
                Outcome::Fail { reason } => Some(reason.clone()),
            };
            let result = StepResult {
                step: step.label(),
                answer: observed.answer,
                spend: observed.spend,
                elapsed_ms: started.elapsed().as_millis(),
                passed: step_outcome == Outcome::Pass,
                reason,
            };
            observe(&result);
            steps.push(result);
            if step_outcome != Outcome::Pass {
                outcome = step_outcome;
                break;
            }
        }

        Report {
            plan: self.plan.name.clone(),
            model: self.model.clone(),
            steps,
            total_ms: whole.elapsed().as_millis(),
            outcome,
        }
    }

    fn drive(&self, step: &Step, observed: &mut Observed) -> Result<Outcome> {
        let mut phase = match step {
            Step::Act { tool, args } => Phase::Act(Target::Tool {
                tool: tool.clone(),
                args: self.with_udid(args),
            }),
            Step::Assert {
                name,
                question,
                threshold,
            } => Phase::Observe(Inquiry::Assert {
                name,
                question,
                threshold: *threshold,
            }),
            Step::Choose {
                name,
                question,
                then,
                roles,
                threshold,
            } => Phase::Observe(Inquiry::Choose {
                name,
                question,
                then: *then,
                roles,
                threshold: *threshold,
            }),
        };

        loop {
            phase = match phase {
                Phase::Observe(inquiry) => {
                    Phase::Decide(inquiry, Screen::parse(&self.argent.describe(&self.udid)?))
                }
                Phase::Decide(inquiry, screen) => self.decide(inquiry, &screen, observed)?,
                Phase::Act(target) => {
                    self.act(&target)?;
                    Phase::Done(Outcome::Pass)
                }
                Phase::Done(outcome) => return Ok(outcome),
            };
        }
    }

    fn decide<'a>(
        &self,
        inquiry: Inquiry<'a>,
        screen: &Screen,
        observed: &mut Observed,
    ) -> Result<Phase<'a>> {
        match inquiry {
            Inquiry::Assert {
                name,
                question,
                threshold,
            } => {
                let noul = self.ask_noul(name, question, screen, observed)?;
                observed.answer = Some(AnswerSummary {
                    probability: noul,
                    confidence: None,
                    choice: None,
                });
                Ok(match assert_verdict(noul, threshold) {
                    AssertVerdict::Pass => Phase::Done(Outcome::Pass),
                    AssertVerdict::Fail => Phase::Done(Outcome::Fail {
                        reason: format!("noul {noul:.2} is under the threshold {threshold:.2}"),
                    }),
                })
            }
            Inquiry::Choose {
                name,
                question,
                then,
                roles,
                threshold,
            } => {
                let options = self.offer(screen, roles);
                if options.is_empty() {
                    bail!("no interactive element is on screen to choose from");
                }
                let chosen = self.ask_choice(name, question, screen, &options, observed)?;
                observed.answer = Some(AnswerSummary {
                    probability: chosen
                        .probabilities
                        .get(&chosen.choice)
                        .copied()
                        .unwrap_or(0.0),
                    confidence: Some(chosen.confidence),
                    choice: Some(chosen.choice.clone()),
                });
                match choose_target(&chosen, threshold) {
                    ChooseVerdict::Tap(id) => {
                        let (x, y) = options
                            .iter()
                            .find(|(option, _)| option == &id)
                            .ok_or_else(|| anyhow!("the decider chose `{id}`, which is not on screen"))?
                            .1
                            .centre();
                        Ok(match then {
                            ChooseAction::Tap => Phase::Act(Target::Element { x, y }),
                        })
                    }
                    ChooseVerdict::Undecided { reason } => {
                        Ok(Phase::Done(Outcome::Fail { reason }))
                    }
                }
            }
        }
    }

    fn act(&self, target: &Target) -> Result<()> {
        let data = match target {
            Target::Tool { tool, args } => self.argent.call(tool, args.clone())?,
            Target::Element { x, y } => self.argent.call(
                "gesture-tap",
                json!({ "udid": self.udid, "x": x, "y": y }),
            )?,
        };
        match data.get("success").and_then(Value::as_bool) {
            Some(false) => bail!(
                "tool reported success=false: {}",
                data.get("note").and_then(Value::as_str).unwrap_or("no note")
            ),
            _ => Ok(()),
        }
    }

    fn offer<'s>(&self, screen: &'s Screen, roles: &[String]) -> Vec<(String, &'s Element)> {
        screen
            .offerable(roles)
            .into_iter()
            .take(MAX_CHOICE_OPTIONS)
            .enumerate()
            .map(|(index, element)| (format!("e{index}"), element))
            .collect()
    }

    fn ask_noul(
        &self,
        name: &str,
        question: &str,
        screen: &Screen,
        observed: &mut Observed,
    ) -> Result<f64> {
        let asked = Question::Noul {
            instructions: question.to_string(),
            criteria: NoulCriteria {
                yes: "The screen matches what the question describes.".to_string(),
                no: "The screen does not match what the question describes.".to_string(),
            },
        };
        match self.ask(name, asked, screen, observed)? {
            Answer::Noul { noul } => Ok(noul),
            other => bail!("the decider answered `{name}` with {other:?} instead of a noul"),
        }
    }

    fn ask_choice(
        &self,
        name: &str,
        question: &str,
        screen: &Screen,
        options: &[(String, &Element)],
        observed: &mut Observed,
    ) -> Result<ChoiceAnswer> {
        let asked = Question::Choice {
            instructions: question.to_string(),
            criteria: options
                .iter()
                .map(|(id, element)| (id.clone(), element.describe()))
                .collect(),
        };
        match self.ask(name, asked, screen, observed)? {
            Answer::Choice(chosen) => Ok(chosen),
            other => bail!("the decider answered `{name}` with {other:?} instead of a choice"),
        }
    }

    fn ask(
        &self,
        name: &str,
        question: Question,
        screen: &Screen,
        observed: &mut Observed,
    ) -> Result<Answer> {
        let decider = self
            .decider
            .as_ref()
            .map_err(|reason| anyhow!(reason.clone()))?;
        let questions = BTreeMap::from([(name.to_string(), question)]);
        let started = Instant::now();
        let decision = decider.decide(&state(screen), &questions)?;
        observed.spend = Some(Spend {
            decide_ms: started.elapsed().as_millis(),
            input_tokens: decision.usage.input_tokens,
            output_tokens: decision.usage.output_tokens,
            cost: decision.usage.cost,
        });
        Ok(decision.answer(name)?.clone())
    }

    fn with_udid(&self, args: &Value) -> Value {
        let mut object = match args {
            Value::Object(map) => map.clone(),
            _ => Map::new(),
        };
        object
            .entry("udid")
            .or_insert_with(|| Value::String(self.udid.clone()));
        Value::Object(object)
    }
}

fn state(screen: &Screen) -> Value {
    Value::String(screen.state_text())
}

pub fn resolve_device(devices: &[Device], selector: &DeviceSelector) -> Result<String> {
    match selector {
        DeviceSelector::Udid(udid) => Ok(udid.clone()),
        DeviceSelector::Name(name) => devices
            .iter()
            .find(|device| device.is_running() && device.name.contains(name.as_str()))
            .or_else(|| devices.iter().find(|device| device.name.contains(name.as_str())))
            .map(|device| device.udid.clone())
            .ok_or_else(|| anyhow!("no device is named like `{name}`")),
        DeviceSelector::First => devices
            .iter()
            .find(|device| device.is_running())
            .map(|device| device.udid.clone())
            .ok_or_else(|| anyhow!("no device is running; boot one first")),
    }
}
