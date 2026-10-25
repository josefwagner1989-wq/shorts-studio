//! The three thinking roles. Each builds a prompt, sends it through the router
//! (which picks Claude / GPT / Gemini / local per role) and parses JSON back.

use crate::jsonx;
use crate::llm::ChatRequest;
use crate::model::{Critique, Idea, Script};
use crate::router::{Role, Router};
use crate::{Error, Result};

const SYSTEM: &str = "You work on a YouTube Shorts channel. \
Reply with JSON only, no commentary. Platforms demonetize mass-produced, repetitive or \
template-like content, so every output must be specific, useful and original.";

fn ask(router: &Router, role: Role, prompt: String, max_tokens: u32, temperature: f32) -> Result<String> {
    let req = ChatRequest { system: SYSTEM.into(), prompt, max_tokens, temperature };
    Ok(router.complete(role, &req)?.text)
}

pub fn plan_ideas(router: &Router, context: &str, existing: &[String], n: usize) -> Result<Vec<Idea>> {
    let prompt = format!(
        "TASK: IDEAS\nChannel strategy:\n{}\n\nAlready planned (do not repeat):\n{}\n\n\
Propose {n} new short video ideas with distinct, specific angles.\n\
Reply with a JSON array of {{\"title\": string (max 60 chars), \"hook\": string (first spoken line, max 15 words), \
\"angle\": string (one sentence: why it is worth watching and what makes it original), \
\"format\": one of \"tip\",\"myth-vs-fact\",\"story\",\"how-to\",\"comparison\"}}.",
        context,
        if existing.is_empty() { "(none)".to_string() } else { existing.join("\n") },
    );
    let mut ideas: Vec<Idea> = jsonx::extract(&ask(router, Role::Strategist, prompt, 2000, 0.9)?)?;
    ideas.retain(|i| !i.title.trim().is_empty());
    ideas.truncate(n);
    if ideas.is_empty() {
        return Err(Error::Parse("no ideas returned".into()));
    }
    Ok(ideas)
}

pub fn write_script(router: &Router, context: &str, idea: &Idea, fixes: &[String]) -> Result<Script> {
    let prompt = format!(
        "TASK: SCRIPT\nChannel strategy:\n{}\n\nIdea: {}\nHook (use as the first line): {}\nAngle: {}\n{}\n\
Rules: 9:16 vertical, 25-45 seconds total, 4-7 scenes, scene 1 is the hook and lasts at most 3 seconds, \
one idea per scene, on-screen text max 8 words, end with a specific call to action, write in the channel's language. \
The voiceover of each scene must be speakable within that scene's seconds (about 2.5 words per second).\n\
Reply with JSON: {{\"scenes\": [{{\"seconds\": number, \"onScreen\": string, \"voiceover\": string, \"visual\": string}}], \
\"caption\": string (max 150 chars), \"hashtags\": [3-6 strings starting with #]}}",
        context,
        idea.title,
        idea.hook,
        idea.angle,
        if fixes.is_empty() { String::new() } else { format!("Apply these editor fixes:\n- {}", fixes.join("\n- ")) },
    );
    let mut script: Script = jsonx::extract(&ask(router, Role::Writer, prompt, 2500, 0.7)?)?;
    if script.scenes.is_empty() {
        return Err(Error::Parse("script has no scenes".into()));
    }
    for s in &mut script.scenes {
        s.seconds = s.seconds.clamp(1.5, 15.0);
    }
    Ok(script)
}

pub fn critique(router: &Router, context: &str, idea: &Idea, script: &Script) -> Result<Critique> {
    let prompt = format!(
        "TASK: CRITIC\nYou are a strict short-form video editor.\nChannel strategy:\n{}\n\nTitle: {}\nScript: {}\n\n\
Judge hook strength in the first 2 seconds, clarity, pacing, real value to the viewer and originality. Flag anything generic.\n\
Reply with JSON: {{\"score\": integer 1-10, \"originalityRisk\": \"low\"|\"medium\"|\"high\", \"summary\": string, \"fixes\": [up to 4 concrete edits]}}",
        context,
        idea.title,
        serde_json::to_string(script)?,
    );
    let mut c: Critique = jsonx::extract(&ask(router, Role::Critic, prompt, 800, 0.2)?)?;
    c.score = c.score.clamp(1, 10);
    c.fixes.truncate(4);
    Ok(c)
}

/// Critical and legal screening of a finished script, its description, sources and music.
/// A screening to catch problems early — not legal advice.
pub fn legal_review(router: &Router, context: &str, video: &crate::model::Video, script: &Script) -> Result<crate::model::LegalReview> {
    let music = video.music.as_ref().map(|m| m.music_credit()).unwrap_or_else(|| "(no music)".into());
    let prompt = format!(
        "TASK: LEGAL\nYou screen YouTube Shorts for legal and policy risk before publishing. Be strict and concrete; this is a risk screening, not legal advice.\n\
Channel strategy:\n{}\n\nTitle: {}\nScript: {}\nDescription as it will be published:\n{}\nMusic: {}\n\n\
Check: copyright (quoted lyrics, long quotes, copied formats), trademarks and brand names used in a misleading way, \
statements about real people or companies that could be defamatory, health, finance or legal claims that need a source or a disclaimer, \
privacy (identifiable private persons), whether the music license allows this use and whether attribution is present, \
whether factual claims have sources, AI disclosure, and YouTube policy (misleading metadata, spam, mass-produced content, dangerous acts, kids content).\n\
Reply with JSON: {{\"overall\": \"low\"|\"medium\"|\"high\", \"findings\": [{{\"category\": string, \"severity\": \"low\"|\"medium\"|\"high\", \"detail\": string, \"fix\": string}}], \
\"claimsNeedingSources\": [exact claims from the script that need a source but have none]}}. Use an empty findings list when nothing is wrong.",
        context,
        video.idea.as_ref().map(|i| i.title.as_str()).unwrap_or(""),
        serde_json::to_string(script)?,
        crate::model::description(video, script),
        music,
    );
    let mut r: crate::model::LegalReview = jsonx::extract(&ask(router, Role::Critic, prompt, 1500, 0.1)?)?;
    for f in &mut r.findings {
        f.severity = f.severity.to_lowercase();
    }
    r.overall = r.overall.to_lowercase();
    Ok(r)
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct ChapterOutline {
    pub title: String,
    #[serde(default)]
    pub points: Vec<String>,
}

/// Chapter outline for a long video of about `minutes` minutes.
pub fn outline(router: &Router, context: &str, idea: &Idea, minutes: u32) -> Result<Vec<ChapterOutline>> {
    let prompt = format!(
        "TASK: OUTLINE\nChannel strategy:\n{context}\n\nVideo: {}\nHook: {}\nAngle: {}\nTarget length: about {minutes} minutes, 16:9 YouTube video.\n\
Plan 4-8 chapters. The first chapter is a short intro that delivers the hook and says what the viewer will get; the last one wraps up with a specific call to action. \
Each chapter has 2-5 concrete points, in order.\n\
Reply with a JSON array: [{{\"title\": string (max 40 chars, used as a YouTube chapter name), \"points\": [string]}}]",
        idea.title, idea.hook, idea.angle,
    );
    let mut chapters: Vec<ChapterOutline> = jsonx::extract(&ask(router, Role::Strategist, prompt, 2000, 0.6)?)?;
    chapters.retain(|c| !c.title.trim().is_empty());
    if chapters.len() < 3 {
        return Err(Error::Parse(format!("outline has {} chapters; at least 3 are needed", chapters.len())));
    }
    Ok(chapters)
}

/// Writes one chapter's scenes; scenes are tagged with the chapter title.
pub fn write_chapter(router: &Router, context: &str, idea: &Idea, all: &[ChapterOutline], index: usize, seconds: u32) -> Result<Script> {
    let ch = &all[index];
    let toc: Vec<String> = all.iter().enumerate().map(|(i, c)| format!("{}. {}", i + 1, c.title)).collect();
    let prompt = format!(
        "TASK: CHAPTER\nChannel strategy:\n{context}\n\nVideo: {}\nAngle: {}\nAll chapters:\n{}\n\n\
Write chapter {} \"{}\" covering: {}.\nAbout {seconds} seconds of narration, 16:9, one point per scene, on-screen text max 10 words, \
voiceover speakable in the scene's seconds (about 2.5 words per second). Do not repeat other chapters. Write in the channel's language.\n\
Reply with JSON: {{\"scenes\": [{{\"seconds\": number, \"onScreen\": string, \"voiceover\": string, \"visual\": string}}]}}",
        idea.title, idea.angle, toc.join("\n"), index + 1, ch.title, ch.points.join("; "),
    );
    let mut script: Script = jsonx::extract(&ask(router, Role::Writer, prompt, 3000, 0.7)?)?;
    if script.scenes.is_empty() {
        return Err(Error::Parse(format!("chapter {} has no scenes", ch.title)));
    }
    for s in &mut script.scenes {
        s.seconds = s.seconds.clamp(2.0, 30.0);
        s.chapter = ch.title.clone();
    }
    Ok(script)
}

/// Caption and hashtags for a finished long video.
pub fn long_caption(router: &Router, context: &str, idea: &Idea, chapters: &[ChapterOutline]) -> Result<(String, Vec<String>)> {
    #[derive(serde::Deserialize)]
    struct Out {
        caption: String,
        #[serde(default)]
        hashtags: Vec<String>,
    }
    let prompt = format!(
        "TASK: LONGCAPTION\nChannel strategy:\n{context}\n\nVideo: {}\nAngle: {}\nChapters: {}\n\
Write the first lines of the YouTube description (2-3 sentences, what the viewer learns) and 3-6 hashtags.\n\
Reply with JSON: {{\"caption\": string, \"hashtags\": [string]}}",
        idea.title, idea.angle, chapters.iter().map(|c| c.title.as_str()).collect::<Vec<_>>().join(", "),
    );
    let o: Out = jsonx::extract(&ask(router, Role::Writer, prompt, 600, 0.6)?)?;
    Ok((o.caption, o.hashtags))
}
