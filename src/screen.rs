use serde::Serialize;

const INTERACTIVE_ROLES: [&str; 7] = [
    "Button",
    "Cell",
    "Link",
    "TextField",
    "Switch",
    "Slider",
    "MenuItem",
];

const STATE_LIMIT: usize = 12_000;

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct Frame {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Element {
    pub role: String,
    pub label: String,
    pub frame: Frame,
}

impl Element {
    pub fn centre(&self) -> (f64, f64) {
        (
            self.frame.x + self.frame.w / 2.0,
            self.frame.y + self.frame.h / 2.0,
        )
    }

    pub fn is_interactive(&self) -> bool {
        !self.label.is_empty() && INTERACTIVE_ROLES.iter().any(|role| self.role.contains(role))
    }

    pub fn describe(&self) -> String {
        format!("{} \"{}\"", self.role, self.label)
    }
}

#[derive(Debug, Clone)]
pub struct Screen {
    pub raw: String,
    pub elements: Vec<Element>,
}

impl Screen {
    pub fn parse(raw: &str) -> Self {
        let elements = raw.lines().filter_map(parse_line).collect();
        Self {
            raw: raw.to_string(),
            elements,
        }
    }

    pub fn interactive(&self) -> Vec<&Element> {
        self.elements
            .iter()
            .filter(|element| element.is_interactive())
            .collect()
    }

    pub fn state_text(&self) -> String {
        let mut text = String::new();
        for line in self.raw.lines().filter(|line| parse_line(line).is_some()) {
            let trimmed = line.trim();
            if text.len() + trimmed.len() + 1 > STATE_LIMIT {
                break;
            }
            text.push_str(trimmed);
            text.push('\n');
        }
        text
    }
}

fn parse_line(line: &str) -> Option<Element> {
    let trimmed = line.trim_end();
    let body = trimmed.strip_suffix(')')?;
    let open = body.rfind('(')?;
    let numbers = body[open + 1..]
        .split(',')
        .map(|part| part.trim().parse::<f64>())
        .collect::<Result<Vec<f64>, _>>()
        .ok()?;
    let [x, y, w, h] = numbers[..] else {
        return None;
    };

    let head = body[..open].trim_end();
    let (role_part, label) = match head.find('"') {
        Some(quote) => {
            let rest = &head[quote + 1..];
            let close = rest.find('"')?;
            (head[..quote].trim_end(), rest[..close].to_string())
        }
        None => (head, String::new()),
    };
    let role = role_part.split_whitespace().last()?.to_string();

    Some(Element {
        role,
        label,
        frame: Frame { x, y, w, h },
    })
}
