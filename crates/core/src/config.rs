//! Settings live in `$XDG_CONFIG_HOME/shorts-studio/settings.json`
//! (inside Flatpak: `~/.var/app/<app-id>/config/...`).
//! API keys are read from environment variables named in the settings,
//! so the file never holds secrets.

use crate::model::Strategy;
use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ProviderKind {
    Anthropic,
    OpenAi,
    Gemini,
    Ollama,
    /// Deterministic offline provider for tests and dry runs.
    Mock,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    pub name: String,
    pub kind: ProviderKind,
    pub model: String,
    /// Environment variable holding the API key (not needed for Ollama/Mock).
    #[serde(default)]
    pub api_key_env: Option<String>,
    #[serde(default)]
    pub base_url: Option<String>,
    /// Daily token ceiling you set for this key. The router skips the provider
    /// for the rest of the day once it is reached. 0 = no ceiling.
    #[serde(default)]
    pub daily_token_budget: u64,
    /// Switched off in Preferences: the router never calls it.
    #[serde(default = "yes")]
    pub enabled: bool,
}

fn yes() -> bool {
    true
}

fn outbox() -> String {
    "outbox".into()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Roles {
    /// Provider names in order of preference; the router falls back down the list.
    pub strategist: Vec<String>,
    pub writer: Vec<String>,
    pub critic: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase", tag = "engine")]
pub enum VoiceEngine {
    /// Local neural TTS: https://github.com/rhasspy/piper — `model` is the .onnx voice path.
    Piper { model: String },
    /// Local robotic TTS, always available as a fallback.
    Espeak { voice: String },
    /// OpenAI text-to-speech API.
    OpenAi { model: String, voice: String },
    /// Silent track of the scene's length (for tests or captions-only videos).
    None,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenderSettings {
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    pub background: String,
    pub text_color: String,
    pub accent: String,
    /// Optional .ttf/.otf; when empty the renderer searches common font paths.
    #[serde(default)]
    pub font_file: String,
}

impl Default for RenderSettings {
    fn default() -> Self {
        Self {
            width: 1080,
            height: 1920,
            fps: 30,
            background: "0x141922".into(),
            text_color: "white".into(),
            accent: "0x7D93FF".into(),
            font_file: String::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublishSettings {
    /// "outbox" (MP4 + caption into a folder) or "youtube".
    #[serde(default = "outbox")]
    pub target: String,
    /// Publish without a human click when the critic score reaches `min_score`.
    pub auto_publish: bool,
    pub min_score: u8,
    /// "private", "unlisted" or "public". Start with private while testing.
    pub youtube_privacy: String,
    /// Environment variables for the YouTube OAuth client (your own Google Cloud project).
    pub youtube_client_id_env: String,
    pub youtube_client_secret_env: String,
}

impl Default for PublishSettings {
    fn default() -> Self {
        Self {
            target: outbox(),
            auto_publish: false,
            min_score: 7,
            youtube_privacy: "private".into(),
            youtube_client_id_env: "YOUTUBE_CLIENT_ID".into(),
            youtube_client_secret_env: "YOUTUBE_CLIENT_SECRET".into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub strategy: Strategy,
    pub providers: Vec<ProviderConfig>,
    pub roles: Roles,
    pub voice: VoiceEngine,
    #[serde(default)]
    pub render: RenderSettings,
    #[serde(default)]
    pub publish: PublishSettings,
    /// Where projects, audio and videos are written. Empty = XDG data dir.
    #[serde(default)]
    pub data_dir: String,
}

impl Default for Settings {
    fn default() -> Self {
        let p = |name: &str, kind, model: &str, env: Option<&str>| ProviderConfig {
            name: name.into(),
            kind,
            model: model.into(),
            api_key_env: env.map(Into::into),
            base_url: None,
            daily_token_budget: 0,
            enabled: true,
        };
        Self {
            strategy: Strategy::default(),
            providers: vec![
                p("claude", ProviderKind::Anthropic, "claude-sonnet-5-5", Some("ANTHROPIC_API_KEY")),
                p("gpt", ProviderKind::OpenAi, "gpt-5-mini", Some("OPENAI_API_KEY")),
                p("gemini", ProviderKind::Gemini, "gemini-2.5-flash", Some("GEMINI_API_KEY")),
                p("local", ProviderKind::Ollama, "qwen2.5:7b", None),
            ],
            roles: Roles {
                strategist: vec!["claude".into(), "gemini".into(), "local".into()],
                writer: vec!["gpt".into(), "claude".into(), "local".into()],
                critic: vec!["gemini".into(), "claude".into(), "local".into()],
            },
            voice: VoiceEngine::Espeak { voice: "en-us".into() },
            render: RenderSettings::default(),
            publish: PublishSettings::default(),
            data_dir: String::new(),
        }
    }
}

impl Settings {
    pub fn config_path() -> Result<PathBuf> {
        let dirs = directories::ProjectDirs::from("io.github", "shortsstudio", "shorts-studio")
            .ok_or_else(|| Error::Config("no home directory".into()))?;
        Ok(dirs.config_dir().join("settings.json"))
    }

    pub fn data_path(&self) -> Result<PathBuf> {
        if !self.data_dir.is_empty() {
            return Ok(PathBuf::from(&self.data_dir));
        }
        let dirs = directories::ProjectDirs::from("io.github", "shortsstudio", "shorts-studio")
            .ok_or_else(|| Error::Config("no home directory".into()))?;
        Ok(dirs.data_dir().to_path_buf())
    }

    /// Loads settings, writing the defaults on first run.
    pub fn load_or_default() -> Result<Self> {
        let path = Self::config_path()?;
        if path.exists() {
            Ok(serde_json::from_str(&std::fs::read_to_string(&path)?)?)
        } else {
            let s = Self::default();
            s.save()?;
            Ok(s)
        }
    }

    pub fn save(&self) -> Result<()> {
        let path = Self::config_path()?;
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(path, serde_json::to_string_pretty(self)?)?;
        Ok(())
    }
}
