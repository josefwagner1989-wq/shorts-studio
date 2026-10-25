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

/// What a project produces. Old files without a kind load as `Short`.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum ProjectKind {
    #[default]
    Short,
    Long,
    Deck,
    Design,
    Plan,
}

impl ProjectKind {
    pub fn label(self) -> &'static str {
        match self {
            ProjectKind::Short => "Short",
            ProjectKind::Long => "Long video",
            ProjectKind::Deck => "Presentation",
            ProjectKind::Design => "Design",
            ProjectKind::Plan => "Strategy plan",
        }
    }

    /// The format a new project of this kind starts with.
    pub fn default_format(self) -> Format {
        match self {
            ProjectKind::Short => Format::Vertical,
            ProjectKind::Long | ProjectKind::Deck | ProjectKind::Plan => Format::Horizontal,
            ProjectKind::Design => Format::Thumbnail,
        }
    }
}

/// Output canvas. Sizes follow YouTube's and common social specs.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum Format {
    /// 9:16, 1080×1920 — Shorts.
    Vertical,
    /// 16:9, 1920×1080 — long videos and slides.
    Horizontal,
    /// 1:1, 1080×1080.
    Square,
    /// 4:5, 1080×1350 — feed posts.
    Portrait,
    /// 16:9, 1280×720 — YouTube thumbnail.
    Thumbnail,
    /// 2560×1440 — YouTube channel banner.
    Banner,
}

impl Format {
    pub fn size(self) -> (u32, u32) {
        match self {
            Format::Vertical => (1080, 1920),
            Format::Horizontal => (1920, 1080),
            Format::Square => (1080, 1080),
            Format::Portrait => (1080, 1350),
            Format::Thumbnail => (1280, 720),
            Format::Banner => (2560, 1440),
        }
    }

    /// Margin (px) that text should stay inside: covers platform UI overlays.
    pub fn safe_margin(self) -> u32 {
        let (w, h) = self.size();
        match self {
            // Shorts: captions, buttons and channel name overlay the edges.
            Format::Vertical => w / 10,
            // Banner: only the centre 1546×423 is visible on every device.
            Format::Banner => (h - 423) / 2,
            _ => w.min(h) / 20,
        }
    }

    pub fn is_video(self) -> bool {
        matches!(self, Format::Vertical | Format::Horizontal | Format::Square | Format::Portrait)
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
    /// Chapter this scene belongs to (long videos). Empty for Shorts.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub chapter: String,
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
    #[serde(default)]
    pub kind: ProjectKind,
    /// None = the kind's default format.
    #[serde(default)]
    pub format: Option<Format>,
    pub idea: Option<Idea>,
    pub stage: Stage,
    pub script: Option<Script>,
    pub critique: Option<Critique>,
    /// One audio file per scene, in scene order.
    #[serde(default)]
    pub voice_files: Vec<String>,
    pub video_file: Option<String>,
    pub published_url: Option<String>,
    /// Sources for the facts in the video, listed in the description.
    #[serde(default)]
    pub sources: Vec<Source>,
    /// Background track used in the render (credited in the description).
    #[serde(default)]
    pub music: Option<crate::music::Track>,
    #[serde(default)]
    pub legal: Option<LegalReview>,
    /// Real length of each scene after rendering (seconds), for chapter timestamps.
    #[serde(default)]
    pub scene_seconds: Vec<f32>,
    /// Subtitles file (SubRip) written with the render.
    #[serde(default)]
    pub srt_file: Option<String>,
    #[serde(default)]
    pub log: Vec<String>,
    pub created_at: u64,
}

impl Video {
    pub fn format(&self) -> Format {
        self.format.unwrap_or_else(|| self.kind.default_format())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Source {
    pub title: String,
    pub url: String,
}

impl Source {
    /// Parses one "Title | https://…" line (or a bare URL).
    pub fn parse_line(line: &str) -> Option<Source> {
        let line = line.trim();
        if line.is_empty() {
            return None;
        }
        let (title, url) = match line.rsplit_once('|') {
            Some((t, u)) => (t.trim().to_string(), u.trim().to_string()),
            None => (String::new(), line.to_string()),
        };
        if !(url.starts_with("https://") || url.starts_with("http://")) {
            return None;
        }
        Some(Source { title: if title.is_empty() { url.clone() } else { title }, url })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LegalFinding {
    /// copyright, trademark, defamation, health-claim, finance-claim, privacy,
    /// music-license, sources, ai-disclosure, platform-policy, other
    pub category: String,
    /// "low", "medium" or "high"
    pub severity: String,
    pub detail: String,
    pub fix: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LegalReview {
    /// "low", "medium" or "high"
    pub overall: String,
    pub findings: Vec<LegalFinding>,
    /// Claims in the script that need a source before publishing.
    #[serde(default, rename = "claimsNeedingSources", alias = "claims_needing_sources")]
    pub claims_needing_sources: Vec<String>,
}

impl LegalReview {
    pub fn blocks_auto_publish(&self) -> bool {
        self.overall == "high" || self.findings.iter().any(|f| f.severity == "high")
    }
}

fn timestamp(secs: f32) -> String {
    let t = secs.max(0.0).round() as u64;
    if t >= 3600 {
        format!("{}:{:02}:{:02}", t / 3600, (t / 60) % 60, t % 60)
    } else {
        format!("{:02}:{:02}", t / 60, t % 60)
    }
}

/// YouTube chapter lines from scene chapters and rendered scene lengths.
/// Empty unless YouTube would accept them: first at 00:00, at least 3, each at least 10 s.
pub fn chapter_lines(script: &Script, scene_seconds: &[f32]) -> Vec<String> {
    if scene_seconds.len() != script.scenes.len() {
        return Vec::new();
    }
    let mut marks: Vec<(f32, String)> = Vec::new();
    let mut t = 0.0;
    for (scene, len) in script.scenes.iter().zip(scene_seconds) {
        let ch = scene.chapter.trim();
        if !ch.is_empty() && marks.last().map(|m| m.1.as_str()) != Some(ch) {
            marks.push((t, ch.to_string()));
        }
        t += len;
    }
    let total = t;
    if marks.len() < 3 || marks[0].0 > 0.5 {
        return Vec::new();
    }
    let long_enough = marks.iter().enumerate().all(|(i, (start, _))| {
        let end = marks.get(i + 1).map(|m| m.0).unwrap_or(total);
        end - start >= 10.0
    });
    if !long_enough {
        return Vec::new();
    }
    marks.into_iter().map(|(start, title)| format!("{} {title}", timestamp(start))).collect()
}

/// The full description: caption, chapters, sources, music credit, AI disclosure, hashtags.
pub fn description(video: &Video, script: &Script) -> String {
    let mut out = script.caption.trim().to_string();
    let chapters = chapter_lines(script, &video.scene_seconds);
    if !chapters.is_empty() {
        out.push_str("\n\nChapters:\n");
        out.push_str(&chapters.join("\n"));
    }
    if !video.sources.is_empty() {
        out.push_str("\n\nSources:");
        for s in &video.sources {
            out.push_str(&format!("\n• {} – {}", s.title, s.url));
        }
    }
    if let Some(m) = &video.music {
        out.push_str(&format!("\n\n{}", m.music_credit()));
    }
    out.push_str("\n\nMade with AI assistance (script, voice).");
    let tags = script.hashtags.join(" ");
    out.push_str(&format!("\n\n{tags} #Shorts"));
    out.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn old_video_files_load_as_vertical_shorts() {
        let old = r#"{"id":"v1","idea":null,"stage":"rendered","script":null,"critique":null,
                      "video_file":null,"published_url":null,"created_at":1}"#;
        let v: Video = serde_json::from_str(old).unwrap();
        assert_eq!(v.kind, ProjectKind::Short);
        assert_eq!(v.format(), Format::Vertical);
        assert_eq!(v.format().size(), (1080, 1920));
        let long = Video { kind: ProjectKind::Long, ..Default::default() };
        assert_eq!(long.format().size(), (1920, 1080));
        assert_eq!(Format::Banner.safe_margin(), 508);
    }

    #[test]
    fn chapters_follow_youtube_rules() {
        let sc = |ch: &str| Scene { seconds: 0.0, on_screen: String::new(), voiceover: String::new(), visual: String::new(), chapter: ch.into() };
        let script = Script { scenes: vec![sc("Intro"), sc("Intro"), sc("Why"), sc("How")], caption: String::new(), hashtags: vec![] };
        assert_eq!(chapter_lines(&script, &[6.0, 6.0, 15.0, 70.0]), vec!["00:00 Intro", "00:12 Why", "00:27 How"]);
        // a chapter shorter than 10 s → no chapters at all
        assert!(chapter_lines(&script, &[3.0, 3.0, 15.0, 70.0]).is_empty());
        // fewer than 3 chapters → none
        let two = Script { scenes: vec![sc("A"), sc("B")], ..script.clone() };
        assert!(chapter_lines(&two, &[30.0, 30.0]).is_empty());
        assert_eq!(timestamp(3725.0), "1:02:05");
    }

    #[test]
    fn source_lines_and_description() {
        assert_eq!(Source::parse_line("WHO report | https://who.int/x").unwrap().title, "WHO report");
        assert_eq!(Source::parse_line("https://a.b").unwrap().title, "https://a.b");
        assert!(Source::parse_line("not a link").is_none());
        let v = Video {
            sources: vec![Source { title: "Paper".into(), url: "https://p.q".into() }],
            music: Some(crate::music::Track { file: String::new(), title: "Calm".into(), artist: "X".into(), license: "CC0".into(), url: String::new() }),
            ..Default::default()
        };
        let s = Script { scenes: vec![], caption: "Hi".into(), hashtags: vec!["#ai".into()] };
        let d = description(&v, &s);
        assert!(d.contains("Sources:\n• Paper – https://p.q"));
        assert!(d.contains("Music: \"Calm\" by X (CC0)"));
        assert!(d.ends_with("#ai #Shorts"));
    }
}
