use arjev::plan::{ChooseAction, DeviceSelector, Plan, Step};
use std::path::Path;

#[test]
fn the_example_plan_loads_with_its_defaults_applied() {
    let plan = Plan::load(Path::new("examples/settings-general.yaml")).expect("example plan loads");

    assert_eq!(plan.name, "settings-general");
    assert!(matches!(plan.device, DeviceSelector::First));
    assert_eq!(plan.steps.len(), 6);

    let Step::Act { tool, args } = &plan.steps[0] else {
        panic!("the first step launches an app");
    };
    assert_eq!(tool, "launch-app");
    assert_eq!(args["bundleId"], "com.apple.Preferences");

    let Step::Assert { threshold, .. } = &plan.steps[2] else {
        panic!("the third step asserts");
    };
    assert_eq!(*threshold, 0.8);

    let Step::Choose { then, .. } = &plan.steps[3] else {
        panic!("the fourth step chooses");
    };
    assert!(matches!(then, ChooseAction::Tap));
}
