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
    assert!(offered.contains(&"AXButton \"General\"".to_string()));
    assert!(offered.contains(&"AXTextField \"apple.id\"".to_string()));
    assert!(!offered.iter().any(|entry| entry.starts_with("AXStaticText")));
    assert!(!offered.iter().any(|entry| entry.starts_with("AXGroup")));
}

#[test]
fn the_state_sent_to_jev_drops_the_prose_header() {
    let state = screen().state_text();
    assert!(!state.contains("Coordinates are normalized"));
    assert!(state.contains("AXButton \"General\" id=\"com.apple.settings.general\""));
    assert_eq!(state.lines().count(), screen().elements.len());
}
