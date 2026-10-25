//! The three thinking roles. Each builds a prompt, sends it through the router
//! (which picks Claude / GPT / Gemini / local per role) and parses JSON back.

use crate::jsonx;
use crate::llm::ChatRequest;
use crate::model::{Critique, Idea, Script, Strategy};
use crate::router::{Role, Router};
use crate::{Error, Result};

const SYSTEM: &str = "You work on a YouTube Shorts channel. \
Reply with JSON only, no commentary. Platforms demonetize mass-produced, repetitive or \
template-like content, so every output must be specific, useful and original.";

fn ask(router: &Router, role: Role, prompt: String, max_tokens: u32, temperature: f32) -> Result<String> {
    let req = ChatRequest { system: SYSTEM.into(), prompt, max_tokens, temperature };
    Ok(router.complete(role, &req)?.text)
}

pub fn plan_ideas(router: &Router, strategy: &Strategy, existing: &[String], n: usize) -> Result<Vec<Idea>> {
    let prompt = format!(
        "TASK: IDEAS\nChannel strategy:\n{}\n\nAlready planned (do not repeat):\n{}\n\n\
Propose {n} new short video ideas with distinct, specific angles.\n\
Reply with a JSON array of {{\"title\": string (max 60 chars), \"hook\": string (first spoken line, max 15 words), \
\"angle\": string (one sentence: why it is worth watching and what makes it original), \
\"format\": one of \"tip\",\"myth-vs-fact\",\"story\",\"how-to\",\"comparison\"}}.",
        strategy.as_prompt(),
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

pub fn write_script(router: &Router, strategy: &Strategy, idea: &Idea, fixes: &[String]) -> Result<Script> {
    let prompt = format!(
        "TASK: SCRIPT\nChannel strategy:\n{}\n\nIdea: {}\nHook (use as the first line): {}\nAngle: {}\n{}\n\
Rules: 9:16 vertical, 25-45 seconds total, 4-7 scenes, scene 1 is the hook and lasts at most 3 seconds, \
one idea per scene, on-screen text max 8 words, end with a specific call to action, write in the channel's language. \
The voiceover of each scene must be speakable within that scene's seconds (about 2.5 words per second).\n\
Reply with JSON: {{\"scenes\": [{{\"seconds\": number, \"onScreen\": string, \"voiceover\": string, \"visual\": string}}], \
\"caption\": string (max 150 chars), \"hashtags\": [3-6 strings starting with #]}}",
        strategy.as_prompt(),
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

pub fn critique(router: &Router, strategy: &Strategy, idea: &Idea, script: &Script) -> Result<Critique> {
    let prompt = format!(
        "TASK: CRITIC\nYou are a strict short-form video editor.\nChannel strategy:\n{}\n\nTitle: {}\nScript: {}\n\n\
Judge hook strength in the first 2 seconds, clarity, pacing, real value to the viewer and originality. Flag anything generic.\n\
Reply with JSON: {{\"score\": integer 1-10, \"originalityRisk\": \"low\"|\"medium\"|\"high\", \"summary\": string, \"fixes\": [up to 4 concrete edits]}}",
        strategy.as_prompt(),
        idea.title,
        serde_json::to_string(script)?,
    );
    let mut c: Critique = jsonx::extract(&ask(router, Role::Critic, prompt, 800, 0.2)?)?;
    c.score = c.score.clamp(1, 10);
    c.fixes.truncate(4);
    Ok(c)
}
