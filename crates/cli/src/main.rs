//! `shorts` — headless Shorts Studio.
//!
//!   shorts selftest [DIR]          offline end-to-end run (mock models) → 9:16 Short in DIR
//!   shorts selftest-long [DIR]     same for a 16:9 long video with chapters
//!   shorts plan [N]                ask the strategist for N ideas
//!   shorts list                    show the board
//!   shorts run <ID|all> [--publish youtube|outbox]
//!   shorts approve <ID>
//!   shorts legal <ID>              critical and legal screening
//!   shorts connect-youtube         one-time Google sign-in

use shorts_core::config::{ProviderConfig, ProviderKind, Settings, VoiceEngine};
use shorts_core::model::{ProjectKind, Stage};
use shorts_core::publish::{Outbox, Publisher, YouTube};
use shorts_core::router::Router;
use shorts_core::store::Store;
use shorts_core::{agents, voice, workflow, Result};
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match run(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

fn publisher_from(args: &[String], s: &Settings, store: &Store) -> Result<Option<Box<dyn Publisher>>> {
    let pos = args.iter().position(|a| a == "--publish");
    Ok(match pos.and_then(|p| args.get(p + 1)).map(String::as_str) {
        Some("youtube") => Some(Box::new(YouTube::from_settings(&s.publish, store.root())?)),
        Some("outbox") => Some(Box::new(Outbox { dir: store.root().join("outbox") })),
        Some(other) => return Err(shorts_core::Error::Config(format!("unknown publisher {other}"))),
        None => None,
    })
}

fn run(args: &[String]) -> Result<()> {
    let cmd = args.first().map(String::as_str).unwrap_or("help");
    match cmd {
        "selftest" => selftest(args.get(1).map(String::as_str), ProjectKind::Short),
        "selftest-long" => selftest(args.get(1).map(String::as_str), ProjectKind::Long),
        "plan" => {
            let s = Settings::load_or_default()?;
            let store = Store::open(&s.data_path()?)?;
            let router = Router::from_settings(&s)?;
            let n = args.get(1).and_then(|x| x.parse().ok()).unwrap_or(5);
            let existing: Vec<String> = store.list()?.iter().filter_map(|v| v.idea.as_ref().map(|i| i.title.clone())).collect();
            for idea in agents::plan_ideas(&router, &s.context(), &existing, n)? {
                let v = workflow::new_video(&store, idea)?;
                println!("{}  {}", v.id, v.idea.unwrap().title);
            }
            for t in router.trace.lock().unwrap().iter() {
                eprintln!("  · {t}");
            }
            Ok(())
        }
        "list" => {
            let s = Settings::load_or_default()?;
            let store = Store::open(&s.data_path()?)?;
            for v in store.list()? {
                let score = v.critique.as_ref().map(|c| format!("{}/10", c.score)).unwrap_or_else(|| "–".into());
                println!("{:<18} {:<10} {:>5}  {}", v.id, v.stage.label(), score, v.idea.map(|i| i.title).unwrap_or_default());
            }
            Ok(())
        }
        "run" => {
            let s = Settings::load_or_default()?;
            let store = Store::open(&s.data_path()?)?;
            let router = Router::from_settings(&s)?;
            let publisher = publisher_from(args, &s, &store)?;
            let target = args.get(1).ok_or_else(|| shorts_core::Error::Config("usage: shorts run <ID|all>".into()))?;
            let videos = if target == "all" {
                store.list()?.into_iter().filter(|v| !matches!(v.stage, Stage::Published)).collect()
            } else {
                vec![store.load(target)?]
            };
            let mut failed = 0;
            for v in videos {
                let id = v.id.clone();
                println!("▶ {id}");
                match workflow::run_video(&s, &router, &store, publisher.as_deref(), v, Default::default(), &|m| println!("  {m}")) {
                    Ok(done) => println!("  → {}", done.stage.label()),
                    Err(e) => {
                        failed += 1;
                        println!("  ✗ {e}")
                    }
                }
            }
            if failed > 0 {
                return Err(shorts_core::Error::Config(format!("{failed} video(s) stopped; see `shorts list`")));
            }
            Ok(())
        }
        "approve" => {
            let s = Settings::load_or_default()?;
            let store = Store::open(&s.data_path()?)?;
            let id = args.get(1).ok_or_else(|| shorts_core::Error::Config("usage: shorts approve <ID>".into()))?;
            let v = workflow::approve(&store, store.load(id)?)?;
            println!("{} approved", v.id);
            Ok(())
        }
        "legal" => {
            let s = Settings::load_or_default()?;
            let store = Store::open(&s.data_path()?)?;
            let router = Router::from_settings(&s)?;
            let id = args.get(1).ok_or_else(|| shorts_core::Error::Config("usage: shorts legal <ID>".into()))?;
            let v = workflow::check_legal(&s, &router, &store, store.load(id)?)?;
            let r = v.legal.unwrap();
            println!("Overall risk: {} (screening, not legal advice)", r.overall);
            for f in r.findings {
                println!("- [{}] {}: {}\n    fix: {}", f.severity, f.category, f.detail, f.fix);
            }
            for c in r.claims_needing_sources {
                println!("- needs a source: {c}");
            }
            Ok(())
        }
        "connect-youtube" => {
            let s = Settings::load_or_default()?;
            let yt = YouTube::from_settings(&s.publish, &s.data_path()?)?;
            yt.connect(|url| {
                println!("Open this link to sign in:\n{url}");
                let _ = std::process::Command::new("xdg-open").arg(url).spawn();
            })?;
            println!("YouTube connected.");
            Ok(())
        }
        _ => {
            println!("shorts selftest [DIR] | plan [N] | list | run <ID|all> [--publish youtube|outbox] | approve <ID> | legal <ID> | connect-youtube");
            Ok(())
        }
    }
}

/// Offline end-to-end check used by CI and the daily test run.
fn selftest(dir: Option<&str>, kind: ProjectKind) -> Result<()> {
    let root = std::path::PathBuf::from(dir.unwrap_or("selftest-out"));
    let mut s = Settings {
        data_dir: root.display().to_string(),
        providers: vec![ProviderConfig {
            name: "mock".into(), kind: ProviderKind::Mock, model: "mock".into(),
            api_key_env: None, base_url: None, daily_token_budget: 0, enabled: true,
        }],
        long_minutes: 1,
        ..Default::default()
    };
    s.roles.strategist = vec!["mock".into()];
    s.roles.writer = vec!["mock".into()];
    s.roles.critic = vec!["mock".into()];
    s.publish.auto_publish = true;
    s.voice = if voice::tool_exists("espeak-ng") || voice::tool_exists("espeak") {
        VoiceEngine::Espeak { voice: "en-us".into() }
    } else {
        VoiceEngine::None
    };

    shorts_core::render::check_ffmpeg()?;
    // A generated test tone stands in for a royalty-free track, with its license sidecar.
    let music_dir = root.join("music");
    std::fs::create_dir_all(&music_dir)?;
    let tone = music_dir.join("test-tone.ogg");
    let ok = std::process::Command::new("ffmpeg")
        .args(["-y", "-loglevel", "error", "-f", "lavfi", "-i", "sine=frequency=220:duration=8", "-c:a", "libvorbis"])
        .arg(&tone)
        .status()
        .map(|st| st.success())
        .unwrap_or(false);
    if ok {
        std::fs::write(music_dir.join("test-tone.ogg.json"),
            r#"{"title":"Test tone","artist":"Shorts Studio","license":"CC0","url":"https://creativecommons.org/publicdomain/zero/1.0/"}"#)?;
        s.music.enabled = true;
        s.music.folder = music_dir.display().to_string();
    }

    let store = Store::open(&root)?;
    let router = Router::from_settings(&s)?;
    let ideas = agents::plan_ideas(&router, &s.context(), &[], 1)?;
    let v = workflow::new_project(&store, ideas[0].clone(), kind)?;
    let outbox = Outbox { dir: root.join("outbox") };
    let done = workflow::run_video(&s, &router, &store, Some(&outbox), v, Default::default(), &|m| println!("  {m}"))?;
    let file = done.video_file.clone().unwrap_or_default();
    let secs = voice::duration(std::path::Path::new(&file)).unwrap_or(0.0);
    let size = video_size(&file);
    let chapters = done.script.as_ref().map(|sc| shorts_core::model::chapter_lines(sc, &done.scene_seconds)).unwrap_or_default();
    println!(
        "{}",
        serde_json::json!({"ok": done.stage == Stage::Published, "stage": done.stage.label(), "video": file,
                           "seconds": secs, "voice": format!("{:?}", s.voice), "published": done.published_url,
                           "music": done.music.as_ref().map(|m| m.music_credit()),
                           "legal": done.legal.as_ref().map(|l| l.overall.clone()),
                           "kind": kind.label(), "size": size, "chapters": chapters})
    );
    let (want_secs, want_size) = match kind {
        ProjectKind::Long => (60.0, "1920x1080"),
        _ => (5.0, "1080x1920"),
    };
    if done.stage != Stage::Published || secs < want_secs || size != want_size {
        return Err(shorts_core::Error::Config(format!(
            "selftest failed: stage {}, {secs:.1} s (need {want_secs}), size {size} (need {want_size})", done.stage.label())));
    }
    if kind == ProjectKind::Long && chapters.len() < 3 {
        return Err(shorts_core::Error::Config(format!("selftest failed: expected at least 3 chapters, got {chapters:?}")));
    }
    Ok(())
}

/// "WIDTHxHEIGHT" of the first video stream, via ffprobe.
fn video_size(file: &str) -> String {
    std::process::Command::new("ffprobe")
        .args(["-v", "error", "-select_streams", "v:0", "-show_entries", "stream=width,height", "-of", "csv=s=x:p=0", file])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default()
}
