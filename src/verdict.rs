use crate::jev::ChoiceAnswer;

#[derive(Debug, Clone, PartialEq)]
pub enum ChooseVerdict {
    Tap(String),
    Undecided { reason: String },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AssertVerdict {
    Pass,
    Fail,
}

pub fn assert_verdict(noul: f64, threshold: f64) -> AssertVerdict {
    if noul >= threshold {
        AssertVerdict::Pass
    } else {
        AssertVerdict::Fail
    }
}

const MIN_MARGIN: f64 = 0.2;

pub fn choose_target(answer: &ChoiceAnswer, min_confidence: f64) -> ChooseVerdict {
    let mut ranked: Vec<(&String, f64)> = answer
        .probabilities
        .iter()
        .map(|(id, p)| (id, *p))
        .collect();
    ranked.sort_by(|a, b| b.1.total_cmp(&a.1));
    let winner = ranked.first().map(|(_, p)| *p).unwrap_or(0.0);
    let runner_up = ranked.get(1).map(|(_, p)| *p).unwrap_or(0.0);
    let standings = ranked
        .iter()
        .take(2)
        .map(|(id, p)| format!("{id}={p:.2}"))
        .collect::<Vec<_>>()
        .join(", ");
    if answer.confidence < min_confidence {
        return ChooseVerdict::Undecided {
            reason: format!(
                "confidence {:.2} below {min_confidence:.2}; top: {standings}",
                answer.confidence
            ),
        };
    }
    if winner - runner_up < MIN_MARGIN {
        return ChooseVerdict::Undecided {
            reason: format!("margin {:.2} below {MIN_MARGIN:.2}; top: {standings}", winner - runner_up),
        };
    }
    ChooseVerdict::Tap(answer.choice.clone())
}
