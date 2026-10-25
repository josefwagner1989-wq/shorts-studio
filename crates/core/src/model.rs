use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Strategy {
    pub niche: String,
    pub audience: String,
    pub tone: String,
    pub language: String,
    pub cadence: String,
    /// What makes this channel original — fed to every prompt so output is not generic.
    pub originality: String,
}

impl Strategy {
    pub fn as_prompt(&self) -> String {
        let f = |s: &str| if s.trim().is_empty() { "(not set)".to_string() } else { s.to_string() };
        format!(
            "niche: {}\naudience: {}\ntone: {}\nlanguage: {}\ncadence: {}\noriginality: {}",
            f(&self.niche), f(&self.audience), f(&self.tone), f(&self.language), f(&self.cadence), f(&self.originality)
        )
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum Stage {
    #[default]
    Idea,
    Script,
    Voiced,
    Rendered,
    Approved,
    Published,
    Failed,
}

impl Stage {
    pub const BOARD: [Stage; 6] =
        [Stage::Idea, Stage::Script, Stage::Voiced, Stage::Rendered, Stage::Approved, Stage::Published];
    pub fn label(self) -> &'static str {
        match self {
            Stage::Idea => "Ideas",
            Stage::Script => "Scripted",
            Stage::Voiced => "Voiced",
            Stage::Rendered => "Rendered",
            Stage::Approved => "Approved",
            Stage::Published => "Published",
            Stage::Failed => "Needs attention",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Idea {
    pub title: String,
    pub hook: String,
    pub angle: String,
    #[serde(default)]
    pub format: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Scene {
    pub seconds: f32,
    #[serde(rename = "onScreen", alias = "on_screen")]
    pub on_screen: String,
    pub voiceover: String,
    #[serde(default)]
    pub visual: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Script {
    pub scenes: Vec<Scene>,
    #[serde(default)]
    pub caption: String,
    #[serde(default)]
    pub hashtags: Vec<String>,
}

impl Script {
    pub fn total_seconds(&self) -> f32 {
        self.scenes.iter().map(|s| s.seconds).sum()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Critique {
    pub score: u8,
    #[serde(rename = "originalityRisk", alias = "originality_risk")]
    pub originality_risk: String,
    pub summary: String,
    #[serde(default)]
    pub fixes: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Video {
    pub id: String,
    pub idea: Option<Idea>,
    pub stage: Stage,
    pub script: Option<Script>,
    pub critique: Option<Critique>,
    /// One audio file per scene, in scene order.
    #[serde(default)]
    pub voice_files: Vec<String>,
    pub video_file: Option<String>,
    pub published_url: Option<String>,
    #[serde(default)]
    pub log: Vec<String>,
    pub created_at: u64,
}
