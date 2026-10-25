//! Voiceover: one WAV per scene. Piper (local neural) → espeak-ng (local) → OpenAI TTS → silence.

use crate::config::VoiceEngine;
use crate::model::Script;
use crate::{Error, Result};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

pub fn tool_exists(name: &str) -> bool {
    Command::new(name).arg("--help").stdout(Stdio::null()).stderr(Stdio::null()).status().is_ok()
}

/// Writes `scene-NN.wav` files into `dir` and returns their paths in scene order.
pub fn synthesize(engine: &VoiceEngine, script: &Script, dir: &Path) -> Result<Vec<PathBuf>> {
    std::fs::create_dir_all(dir)?;
    let mut out = Vec::new();
    for (i, scene) in script.scenes.iter().enumerate() {
        let path = dir.join(format!("scene-{i:02}.wav"));
        match engine {
            VoiceEngine::Piper { model } => piper(model, &scene.voiceover, &path)?,
            VoiceEngine::Espeak { voice } => espeak(voice, &scene.voiceover, &path)?,
            VoiceEngine::OpenAi { model, voice } => openai_tts(model, voice, &scene.voiceover, &path)?,
            VoiceEngine::None => silence(scene.seconds, &path)?,
        }
        out.push(path);
    }
    Ok(out)
}

fn run(tool: &str, mut cmd: Command, stdin: Option<&str>) -> Result<()> {
    cmd.stdin(if stdin.is_some() { Stdio::piped() } else { Stdio::null() })
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    let mut child = cmd.spawn().map_err(|e| Error::Tool { tool: tool.into(), message: e.to_string() })?;
    if let (Some(text), Some(mut si)) = (stdin, child.stdin.take()) {
        si.write_all(text.as_bytes())?;
    }
    let o = child.wait_with_output()?;
    if !o.status.success() {
        return Err(Error::Tool { tool: tool.into(), message: String::from_utf8_lossy(&o.stderr).chars().take(400).collect() });
    }
    Ok(())
}

fn piper(model: &str, text: &str, out: &Path) -> Result<()> {
    let mut c = Command::new("piper");
    c.args(["--model", model, "--output_file"]).arg(out);
    run("piper", c, Some(text))
}

fn espeak(voice: &str, text: &str, out: &Path) -> Result<()> {
    let bin = if tool_exists("espeak-ng") { "espeak-ng" } else { "espeak" };
    let mut c = Command::new(bin);
    c.args(["-v", voice, "-s", "165", "-w"]).arg(out).arg(text);
    run(bin, c, None)
}

pub fn silence(seconds: f32, out: &Path) -> Result<()> {
    let mut c = Command::new("ffmpeg");
    c.args(["-y", "-loglevel", "error", "-f", "lavfi", "-i", "anullsrc=r=44100:cl=mono", "-t"])
        .arg(format!("{seconds:.2}"))
        .arg(out);
    run("ffmpeg", c, None)
}

fn openai_tts(model: &str, voice: &str, text: &str, out: &Path) -> Result<()> {
    let key = std::env::var("OPENAI_API_KEY").map_err(|_| Error::Config("set OPENAI_API_KEY for OpenAI voice".into()))?;
    let resp = reqwest::blocking::Client::new()
        .post("https://api.openai.com/v1/audio/speech")
        .bearer_auth(key)
        .json(&serde_json::json!({"model": model, "voice": voice, "input": text, "response_format": "wav"}))
        .send()?;
    if !resp.status().is_success() {
        return Err(Error::Tool { tool: "openai-tts".into(), message: resp.text().unwrap_or_default() });
    }
    std::fs::write(out, resp.bytes()?)?;
    Ok(())
}

/// Length of an audio file in seconds, via ffprobe.
pub fn duration(path: &Path) -> Result<f32> {
    let o = Command::new("ffprobe")
        .args(["-v", "error", "-show_entries", "format=duration", "-of", "default=nw=1:nk=1"])
        .arg(path)
        .output()
        .map_err(|e| Error::Tool { tool: "ffprobe".into(), message: e.to_string() })?;
    String::from_utf8_lossy(&o.stdout)
        .trim()
        .parse()
        .map_err(|_| Error::Tool { tool: "ffprobe".into(), message: format!("no duration for {}", path.display()) })
}
