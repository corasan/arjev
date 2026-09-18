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

/**
 * Turns a Jev choice answer into the runner's decision to tap or to stop.
 *
 * @param answer.choice the winning option key, always one of the element ids offered as criteria
 * @param answer.probabilities every offered element id mapped to its probability; the values sum to 1
 * @param answer.confidence Jev's own confidence in the winning option, 0 to 1
 * @param min_confidence the floor the plan requires before a tap is allowed
 * @returns Tap with the element id to press, or Undecided with a reason recorded in the report
 */
#[allow(unused_variables)]
pub fn choose_target(answer: &ChoiceAnswer, min_confidence: f64) -> ChooseVerdict {
    todo!("owner writes the confidence policy")
}
