use regex::Regex;
use url::Url;

#[derive(Clone, Debug, PartialEq)]
pub struct Palette {
    pub background: String,
    pub foreground: String,
    pub accent: String,
    pub muted: String,
    pub lighter_background: String,
    pub mode: String,
}

pub fn navigation_allowed(input: &str) -> bool {
    !input.chars().any(char::is_control)
        && Url::parse(input)
            .is_ok_and(|u| matches!(u.scheme(), "http" | "https") && u.host_str().is_some())
}

pub fn address(input: &str) -> Option<String> {
    let input = input.trim();
    if input.is_empty() || input.chars().any(char::is_control) {
        return None;
    }
    let has_scheme = Regex::new(r"(?i)^[a-z][a-z\d+.-]*:")
        .unwrap()
        .is_match(input);
    let input = if has_scheme {
        input.to_owned()
    } else {
        format!("https://{input}")
    };
    navigation_allowed(&input).then(|| Url::parse(&input).unwrap().to_string())
}

pub fn theme(source: &str) -> Palette {
    let mut p = Palette {
        background: "#1a1b26".into(),
        foreground: "#c0caf5".into(),
        accent: "#7aa2f7".into(),
        muted: "#565f89".into(),
        lighter_background: "#24283b".into(),
        mode: "dark".into(),
    };
    let values = Regex::new(r#"(?m)^\s*(\w+)\s*=\s*["']([^"']+)["']"#).unwrap();
    for c in values.captures_iter(source) {
        let (key, value) = (&c[1], &c[2]);
        if key == "mode" {
            if matches!(value, "dark" | "light") {
                p.mode = value.into();
            }
            continue;
        }
        if value.len() != 7
            || !value.starts_with('#')
            || !value[1..].bytes().all(|b| b.is_ascii_hexdigit())
        {
            continue;
        }
        match key {
            "background" => p.background = value.into(),
            "foreground" => p.foreground = value.into(),
            "accent" => p.accent = value.into(),
            "muted" => p.muted = value.into(),
            "lighter_background" => p.lighter_background = value.into(),
            _ => {}
        }
    }
    p
}

#[derive(Default)]
pub struct Prompt {
    pub open: bool,
    pub draft: String,
    pub error: String,
    pub failed_target: Option<String>,
    load_error: bool,
}
impl Prompt {
    pub fn show(&mut self, current: &str) {
        if !self.open {
            self.draft = self.failed_target.as_deref().unwrap_or(current).into();
        }
        self.open = true;
    }
    pub fn failed(&mut self, target: &str, message: &str) {
        self.failed_target = Some(target.into());
        self.show(target);
        self.load_error = true;
        self.error = format!("Load failed: {message}. Ctrl+R retry or enter another address.");
    }
    pub fn submit(&mut self) -> Option<String> {
        let target = address(&self.draft);
        if target.is_some() {
            self.failed_target = None;
            self.hide();
        } else {
            self.open = true;
            self.error = "Enter a valid http:// or https:// address.".into();
        }
        target
    }
    pub fn hide(&mut self) {
        self.open = false;
        self.error.clear();
        self.load_error = false;
    }
    pub fn retry_target(&self) -> Option<String> {
        self.failed_target.clone()
    }
    pub fn succeeded(&mut self) {
        self.failed_target = None;
        if self.load_error {
            self.hide();
        }
    }
}

pub mod tabs;
pub mod theme_watch;
