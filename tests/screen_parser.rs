use arjev::screen::{Frame, Screen};

const FIXTURE: &str = include_str!("fixtures/describe-settings.txt");

fn screen() -> Screen {
    Screen::parse(FIXTURE)
}

#[test]
fn header_prose_is_not_mistaken_for_an_element() {
    let screen = screen();
    assert!(!screen
        .elements
        .iter()
        .any(|element| element.role.starts_with("normalized") || element.role == "pixels."));
    assert_eq!(screen.elements.len(), 17);
}

#[test]
fn a_labelled_row_keeps_its_role_label_and_frame() {
    let screen = screen();
    let general = screen
        .elements
        .iter()
        .find(|element| element.role == "AXButton" && element.label == "General")
        .expect("the General row is in the fixture");
    assert_eq!(
        general.frame,
        Frame {
            x: 0.045,
            y: 0.307,
            w: 0.909,
            h: 0.054
        }
    );
    assert_eq!(general.centre(), (0.045 + 0.909 / 2.0, 0.307 + 0.054 / 2.0));
}

#[test]
fn trailing_attributes_do_not_leak_into_the_label() {
    let screen = screen();
    let scroll_bar = screen
        .elements
        .iter()
        .find(|element| element.label.starts_with("Vertical scroll bar"))
        .expect("the scroll bar is in the fixture");
    assert_eq!(scroll_bar.label, "Vertical scroll bar, 2 pages");
    assert_eq!(scroll_bar.role, "AXGroup");
}

#[test]
fn an_unlabelled_root_row_parses_with_an_empty_label() {
    let screen = screen();
    let root = &screen.elements[0];
    assert_eq!(root.role, "AXGroup");
    assert_eq!(root.label, "");
}

#[test]
fn only_actionable_roles_with_a_label_are_offered_as_choices() {
    let screen = screen();
    let offered: Vec<String> = screen
        .interactive()
        .iter()
        .map(|element| element.describe())
        .collect();
    assert!(offered.contains(&"AXButton \"General\" id=com.apple.settings.general".to_string()));
    assert!(offered.contains(&"AXTextField \"apple.id\" id=apple.id".to_string()));
    assert!(!offered.iter().any(|entry| entry.starts_with("AXStaticText")));
    assert!(!offered.iter().any(|entry| entry.starts_with("AXGroup")));
}

#[test]
fn a_group_role_is_offered_only_when_the_step_asks_for_it() {
    let screen = Screen::parse(
        "  AXGroup \"Tab Bar\"  (0.000, 0.905, 1.000, 0.095)\n  AXGroup \"CNBC, U.S. to build, 10 minutes ago\"  (0.040, 0.700, 0.448, 0.200)\n  AXButton \"Today\" id=\"BackButton\"  (0.040, 0.071, 0.109, 0.050)\n",
    );
    let groups: Vec<String> = screen
        .offerable(&["Group".to_string()])
        .iter()
        .map(|element| element.describe())
        .collect();
    assert_eq!(groups, ["AXGroup \"CNBC, U.S. to build, 10 minutes ago\""]);
    assert_eq!(
        screen.interactive().iter().map(|element| element.describe()).collect::<Vec<_>>(),
        ["AXButton \"Today\" id=BackButton"]
    );
}

#[test]
fn an_element_clipped_by_the_screen_edge_is_never_offered() {
    let screen = Screen::parse(
        "  AXGroup \"Today Feed\"  (0.000, 0.000, 1.000, 1.000)\n  AXGroup \"USA TODAY, clipped card\"  (0.040, 0.000, 0.920, 0.161)\n  AXGroup \"CNN, card under the collapsed nav bar\"  (0.040, 0.030, 0.920, 0.161)\n  AXGroup \"NBC News, whole card\"  (0.040, 0.183, 0.920, 0.204)\n  AXButton \"Today\"  (0.062, 0.910, 0.184, 0.062)\n",
    );
    let offered: Vec<String> = screen
        .offerable(&["Group".to_string()])
        .iter()
        .map(|element| element.describe())
        .collect();
    assert_eq!(offered, ["AXGroup \"NBC News, whole card\""]);
    assert_eq!(screen.interactive().len(), 1);
}

#[test]
fn the_state_sent_to_jev_drops_the_prose_header() {
    let state = screen().state_text();
    assert!(!state.contains("Coordinates are normalized"));
    assert!(state.contains("AXButton \"General\" id=\"com.apple.settings.general\""));
    assert_eq!(state.lines().count(), screen().elements.len());
}
