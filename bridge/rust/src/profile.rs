//! User profile persistence (`.zero-agent/profile.json`).

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::tui::{ResponseStyle, UserProfile};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileFile {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub style: String,
    #[serde(default)]
    pub style_custom: String,
    #[serde(default)]
    pub about: String,
    #[serde(default)]
    pub onboarding_complete: bool,
}

impl Default for ProfileFile {
    fn default() -> Self {
        Self {
            name: String::new(),
            style: "concise".into(),
            style_custom: String::new(),
            about: String::new(),
            onboarding_complete: false,
        }
    }
}

impl ProfileFile {
    pub fn path_in(data_dir: &Path) -> PathBuf {
        data_dir.join("profile.json")
    }

    pub fn load(data_dir: &Path) -> Self {
        let path = Self::path_in(data_dir);
        if !path.exists() {
            return Self::default();
        }
        std::fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, data_dir: &Path) -> Result<(), String> {
        if let Some(parent) = data_dir.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::create_dir_all(data_dir);
        let path = Self::path_in(data_dir);
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| format!("failed to serialize profile: {e}"))?;
        std::fs::write(&path, json).map_err(|e| format!("failed to write profile: {e}"))
    }

    pub fn to_user_profile(&self) -> UserProfile {
        UserProfile {
            name: self.name.clone(),
            role: String::new(),
            about: self.about.clone(),
            style: parse_style(&self.style, &self.style_custom),
        }
    }

    pub fn from_user_profile(profile: &UserProfile) -> Self {
        let (style, style_custom) = encode_style(&profile.style);
        Self {
            name: profile.name.clone(),
            style,
            style_custom,
            about: profile.about.clone(),
            onboarding_complete: !profile.name.is_empty(),
        }
    }
}

pub fn needs_onboarding(data_dir: &Path) -> bool {
    let file = ProfileFile::load(data_dir);
    !file.onboarding_complete || file.name.is_empty()
}

fn parse_style(style: &str, custom: &str) -> ResponseStyle {
    match style {
        "verbose" => ResponseStyle::Verbose,
        "technical" => ResponseStyle::Technical,
        "non-technical" => ResponseStyle::Persona("non-technical".into()),
        "custom" => ResponseStyle::Persona(if custom.is_empty() {
            "custom".into()
        } else {
            custom.to_string()
        }),
        _ => ResponseStyle::Concise,
    }
}

fn encode_style(style: &ResponseStyle) -> (String, String) {
    match style {
        ResponseStyle::Concise => ("concise".into(), String::new()),
        ResponseStyle::Verbose => ("verbose".into(), String::new()),
        ResponseStyle::Technical => ("technical".into(), String::new()),
        ResponseStyle::Persona(s) if s == "non-technical" => {
            ("non-technical".into(), String::new())
        }
        ResponseStyle::Persona(s) => ("custom".into(), s.clone()),
    }
}

pub fn profile_system_prompt_section(profile: &UserProfile) -> String {
    let mut parts = Vec::new();
    if !profile.name.is_empty() {
        parts.push(format!(
            "The user's name is {}. Address them by name when speaking to them — avoid generic \"you\" when a personal address is more natural.",
            profile.name
        ));
    }
    parts.push(format!(
        "Response style: {}.",
        style_label(&profile.style)
    ));
    if !profile.about.is_empty() {
        parts.push(format!("User preferences and guidelines: {}", profile.about));
    }
    parts.join("\n")
}

fn style_label(style: &ResponseStyle) -> &str {
    match style {
        ResponseStyle::Concise => "concise — short, direct answers",
        ResponseStyle::Verbose => "verbose — thorough explanations",
        ResponseStyle::Technical => "technical — precise, developer-oriented language",
        ResponseStyle::Persona(s) if s == "non-technical" => {
            "non-technical — plain language, minimal jargon"
        }
        ResponseStyle::Persona(s) => s.as_str(),
    }
}
