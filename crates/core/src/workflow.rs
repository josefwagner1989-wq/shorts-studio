//! Chains the steps for one video. Every stage is saved before the next one
//! starts, so a crash or a failed provider resumes where it stopped.

use crate::agents;
use crate::config::Settings;
use crate::model::{Idea, Stage, Video};
use crate::publish::Publisher;
use crate::music;
use crate::render;
use crate::router::Router;
use crate::store::Store;
use crate::voice;
use crate::{Error, Result};

#[derive(Debug, Clone, Copy)]
pub struct RunOptions {
    /// Rewrite once with the critic's fixes when the score is below `min_score`.
    pub revise_below_min: bool,
    /// Go as far as publishing when settings allow it.
    pub allow_publish: bool,
}

impl Default for RunOptions {
    fn default() -> Self {
        Self { revise_below_min: true, allow_publish: true }
    }
}

pub fn new_video(store: &Store, idea: Idea) -> Result<Video> {
    let v = Video { id: Store::new_id(), idea: Some(idea), created_at: Store::now(), ..Default::default() };
    store.save(&v)?;
    Ok(v)
}

/// Advances `video` through script → critic → voice → render → (approve → publish).
/// `progress` receives one short line per step for the UI.
pub fn run_video(
    settings: &Settings,
    router: &Router,
    store: &Store,
    publisher: Option<&dyn Publisher>,
    mut video: Video,
    opts: RunOptions,
    progress: &dyn Fn(&str),
) -> Result<Video> {
    let idea = video.idea.clone().ok_or_else(|| Error::Config("video has no idea".into()))?;
    let step = |v: &mut Video, msg: String| {
        progress(&msg);
        v.log.push(msg);
    };

    let result: Result<()> = (|| {
        if video.script.is_none() || video.stage == Stage::Idea {
            step(&mut video, "Writing script".into());
            let script = agents::write_script(router, &settings.strategy, &idea, &[])?;
            video.script = Some(script);
            video.stage = Stage::Script;
            store.save(&video)?;
        }

        if video.critique.is_none() {
            step(&mut video, "Running critic".into());
            let mut c = agents::critique(router, &settings.strategy, &idea, video.script.as_ref().unwrap())?;
            if opts.revise_below_min && c.score < settings.publish.min_score && !c.fixes.is_empty() {
                step(&mut video, format!("Critic scored {}/10, rewriting with fixes", c.score));
                let script = agents::write_script(router, &settings.strategy, &idea, &c.fixes)?;
                c = agents::critique(router, &settings.strategy, &idea, &script)?;
                video.script = Some(script);
            }
            step(&mut video, format!("Critic score {}/10 ({} originality risk)", c.score, c.originality_risk));
            video.critique = Some(c);
            store.save(&video)?;
        }

        let dir = store.video_dir(&video.id);
        if video.stage == Stage::Script {
            step(&mut video, "Recording voiceover".into());
            let files = voice::synthesize(&settings.voice, video.script.as_ref().unwrap(), &dir.join("voice"))?;
            video.voice_files = files.iter().map(|p| p.display().to_string()).collect();
            video.stage = Stage::Voiced;
            store.save(&video)?;
        }

        if video.stage == Stage::Voiced {
            if settings.music.enabled && video.music.is_none() {
                match music::scan(std::path::Path::new(&settings.music.folder)) {
                    Ok(tracks) => match music::pick(&tracks, &video.id) {
                        Some(t) => {
                            step(&mut video, format!("Music: {}", t.credit()));
                            video.music = Some(t.clone());
                        }
                        None => step(&mut video, "No licensed tracks in the music folder; rendering without music".into()),
                    },
                    Err(e) => step(&mut video, format!("Music skipped: {e}")),
                }
            }
            step(&mut video, format!("Rendering {:?} {}×{}", video.format(), video.format().size().0, video.format().size().1));
            let voices: Vec<_> = video.voice_files.iter().map(std::path::PathBuf::from).collect();
            let bed = video.music.as_ref().map(|t| (t, &settings.music));
            let mut style = settings.render.clone();
            (style.width, style.height) = video.format().size();
            let out = render::render_short(video.script.as_ref().unwrap(), &voices, &style, bed, &dir.join("work"), &dir.join("short.mp4"))?;
            video.video_file = Some(out.display().to_string());
            video.stage = Stage::Rendered;
            store.save(&video)?;
        }

        if video.stage == Stage::Rendered && video.legal.is_none() {
            step(&mut video, "Checking legal and policy risks".into());
            let r = agents::legal_review(router, &settings.strategy, &video, video.script.as_ref().unwrap())?;
            step(&mut video, format!("Legal check: {} risk, {} finding(s)", r.overall, r.findings.len()));
            video.legal = Some(r);
            store.save(&video)?;
        }

        let score = video.critique.as_ref().map(|c| c.score).unwrap_or(0);
        let legal_block = video.legal.as_ref().is_some_and(|l| l.blocks_auto_publish());
        if video.stage == Stage::Rendered && settings.publish.auto_publish && legal_block {
            step(&mut video, "Auto-publish held: the legal check found a high risk. Review it first.".into());
        }
        if video.stage == Stage::Rendered && settings.publish.auto_publish && !legal_block && score >= settings.publish.min_score {
            step(&mut video, "Auto-approved by score".into());
            video.stage = Stage::Approved;
            store.save(&video)?;
        }

        if video.stage == Stage::Approved && opts.allow_publish {
            if let Some(p) = publisher {
                step(&mut video, format!("Publishing to {}", p.name()));
                let url = p.publish(&video, video.script.as_ref().unwrap(), std::path::Path::new(video.video_file.as_ref().unwrap()))?;
                step(&mut video, format!("Published: {url}"));
                video.published_url = Some(url);
                video.stage = Stage::Published;
                store.save(&video)?;
            }
        }
        Ok(())
    })();

    for line in router.trace.lock().unwrap().drain(..) {
        video.log.push(line);
    }
    if let Err(e) = result {
        video.log.push(format!("Stopped: {e}"));
        store.save(&video)?;
        return Err(e);
    }
    store.save(&video)?;
    Ok(video)
}

/// Marks a rendered video approved by a person.
pub fn approve(store: &Store, mut video: Video) -> Result<Video> {
    if video.stage != Stage::Rendered {
        return Err(Error::Config("only rendered videos can be approved".into()));
    }
    video.stage = Stage::Approved;
    video.log.push("Approved by you".into());
    store.save(&video)?;
    Ok(video)
}

/// Runs (or re-runs) only the critical and legal check for a video.
pub fn check_legal(settings: &Settings, router: &Router, store: &Store, mut video: Video) -> Result<Video> {
    let script = video.script.clone().ok_or_else(|| Error::Config("write the script first".into()))?;
    let r = agents::legal_review(router, &settings.strategy, &video, &script)?;
    video.log.push(format!("Legal check: {} risk, {} finding(s)", r.overall, r.findings.len()));
    video.legal = Some(r);
    store.save(&video)?;
    Ok(video)
}
