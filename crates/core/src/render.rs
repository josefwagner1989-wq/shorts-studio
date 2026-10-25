//! Local 9:16 rendering with ffmpeg: one clip per scene (background colour or
//! picture, styled text, progress bar, voiceover), a lossless concat, then an
//! optional royalty-free music bed ducked under the voice.

use crate::config::{MusicSettings, RenderSettings};
use crate::model::Script;
use crate::music::Track;
use crate::voice;
use crate::{Error, Result};
use std::path::{Path, PathBuf};
use std::process::Command;

const FONT_CANDIDATES: &[&str] = &[
    "/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf",
    "/usr/share/fonts/dejavu-sans-fonts/DejaVuSans-Bold.ttf",
    "/usr/share/fonts/TTF/DejaVuSans-Bold.ttf",
    "/usr/share/fonts/cantarell/Cantarell-VF.otf",
    "/usr/share/fonts/opentype/cantarell/Cantarell-Bold.otf",
    "/usr/share/fonts/liberation/LiberationSans-Bold.ttf",
    "/usr/share/fonts/truetype/liberation/LiberationSans-Bold.ttf",
];

pub fn find_font(settings: &RenderSettings) -> Option<String> {
    if !settings.font_file.is_empty() && Path::new(&settings.font_file).exists() {
        return Some(settings.font_file.clone());
    }
    FONT_CANDIDATES.iter().find(|p| Path::new(p).exists()).map(|p| p.to_string())
}

/// Checks that the ffmpeg in use can encode H.264 + AAC and draw text.
/// The Flatpak bundles such a build; elsewhere this explains what is missing.
pub fn check_ffmpeg() -> Result<()> {
    let run = |args: &[&str]| -> Result<String> {
        let o = Command::new("ffmpeg").args(args).output().map_err(|_| Error::Tool {
            tool: "ffmpeg".into(),
            message: "ffmpeg is not installed. Install it, or use the Flatpak, which includes it.".into(),
        })?;
        Ok(String::from_utf8_lossy(&o.stdout).to_string())
    };
    let enc = run(&["-hide_banner", "-encoders"])?;
    let filt = run(&["-hide_banner", "-filters"])?;
    let mut missing = Vec::new();
    if !enc.contains("libx264") {
        missing.push("H.264 encoder (libx264)");
    }
    if !enc.contains(" aac ") {
        missing.push("AAC encoder");
    }
    if !filt.contains("drawtext") {
        missing.push("drawtext filter (libfreetype)");
    }
    if missing.is_empty() {
        Ok(())
    } else {
        Err(Error::Tool { tool: "ffmpeg".into(), message: format!("this ffmpeg lacks: {}", missing.join(", ")) })
    }
}

/// Greedy word wrap so on-screen text stays inside the safe area.
pub fn wrap(text: &str, max_chars: usize) -> String {
    let mut lines: Vec<String> = Vec::new();
    let mut cur = String::new();
    for w in text.split_whitespace() {
        if !cur.is_empty() && cur.chars().count() + 1 + w.chars().count() > max_chars.max(6) {
            lines.push(std::mem::take(&mut cur));
        }
        if !cur.is_empty() {
            cur.push(' ');
        }
        cur.push_str(w);
    }
    if !cur.is_empty() {
        lines.push(cur);
    }
    lines.join("\n")
}

fn ffmpeg(args: &[String]) -> Result<()> {
    let o = Command::new("ffmpeg")
        .args(args)
        .output()
        .map_err(|e| Error::Tool { tool: "ffmpeg".into(), message: e.to_string() })?;
    if !o.status.success() {
        let err = String::from_utf8_lossy(&o.stderr);
        let tail: Vec<&str> = err.lines().rev().take(6).collect();
        return Err(Error::Tool { tool: "ffmpeg".into(), message: tail.into_iter().rev().collect::<Vec<_>>().join("\n") });
    }
    Ok(())
}

fn clean(p: &Path) -> String {
    p.display().to_string().replace(['\'', ':'], "")
}

/// ffmpeg inputs for the background and the video filter that draws the text.
/// Input 0 is always the background; the caller adds audio as input 1.
fn scene_graph(text: &str, index: usize, count: usize, dur: f32, s: &RenderSettings, work: &Path) -> Result<(Vec<String>, String)> {
    let (w, h) = (s.width, s.height);
    let mut inputs: Vec<String> = Vec::new();
    let mut chain: Vec<String> = Vec::new();
    if !s.background_image.is_empty() && Path::new(&s.background_image).exists() {
        inputs.extend(["-loop".into(), "1".into(), "-t".into(), format!("{dur:.2}"), "-i".into(), s.background_image.clone()]);
        chain.push(format!("scale={w}:{h}:force_original_aspect_ratio=increase,crop={w}:{h},setsar=1,fps={}", s.fps));
        let dim = s.background_dim.clamp(0.0, 0.9);
        if dim > 0.0 {
            chain.push(format!("drawbox=x=0:y=0:w=iw:h=ih:color=black@{dim:.2}:t=fill"));
        }
    } else {
        inputs.extend(["-f".into(), "lavfi".into(), "-i".into(), format!("color=c={}:s={w}x{h}:r={}:d={dur:.2}", s.background, s.fps)]);
    }

    let shown = if s.uppercase { text.to_uppercase() } else { text.to_string() };
    let text_file = work.join(format!("text-{index:02}.txt"));
    let font_arg = match find_font(s) {
        Some(f) => format!("fontfile='{}'", clean(Path::new(&f))),
        None => "font='Sans'".to_string(),
    };
    // Size text from the short side so 9:16 and 16:9 read the same; wrap wider on landscape.
    let fontsize = (w.min(h) as f32 * s.font_scale.clamp(0.03, 0.16)) as u32;
    let wrap_at = if w > h { s.wrap_chars * w as usize / h as usize } else { s.wrap_chars };
    std::fs::write(&text_file, wrap(&shown, wrap_at))?;
    let y = match s.text_position.as_str() {
        "top" => format!("{}", h / 7),
        "bottom" => format!("h-text_h-{}", h / 6),
        _ => "(h-text_h)/2".to_string(),
    };
    let boxed = if s.text_box {
        format!(":box=1:boxcolor={}@{:.2}:boxborderw={}", s.text_box_color, s.text_box_opacity.clamp(0.0, 1.0), fontsize / 3)
    } else {
        String::new()
    };
    chain.push(format!(
        "drawtext={font_arg}:textfile='{tf}':fontsize={fontsize}:fontcolor={fg}:line_spacing={ls}:x=(w-text_w)/2:y={y}{boxed}",
        tf = clean(&text_file),
        fg = s.text_color,
        ls = fontsize / 4,
    ));
    if s.progress_bar && count > 0 {
        let bar_w = (w as f32 * (index + 1) as f32 / count as f32) as u32;
        chain.push(format!("drawbox=x=0:y={}:w={bar_w}:h=14:color={}:t=fill", h - 14, s.accent));
    }
    chain.push("format=yuv420p".into());
    Ok((inputs, chain.join(",")))
}

/// Renders one still frame of a scene for the style editor preview.
pub fn preview_frame(text: &str, settings: &RenderSettings, work: &Path, out_png: &Path) -> Result<PathBuf> {
    std::fs::create_dir_all(work)?;
    let work = &std::fs::canonicalize(work)?;
    let (inputs, vf) = scene_graph(text, 1, 3, 1.0, settings, work)?;
    let mut args: Vec<String> = vec!["-y".into(), "-loglevel".into(), "error".into()];
    args.extend(inputs);
    args.extend(["-vf".into(), vf, "-frames:v".into(), "1".into(), out_png.display().to_string()]);
    ffmpeg(&args)?;
    Ok(out_png.to_path_buf())
}

/// Renders `script` with one voice file per scene into `out` (MP4, H.264 + AAC).
/// Each scene lasts at least its scripted seconds and at least as long as its voiceover.
pub fn render_short(
    script: &Script,
    voices: &[PathBuf],
    settings: &RenderSettings,
    music: Option<(&Track, &MusicSettings)>,
    work: &Path,
    out: &Path,
) -> Result<Rendered> {
    if voices.len() != script.scenes.len() {
        return Err(Error::Config("one voice file per scene is required".into()));
    }
    std::fs::create_dir_all(work)?;
    // Absolute paths: ffmpeg resolves concat-list entries relative to the list file.
    let work = &std::fs::canonicalize(work)?;
    let n = script.scenes.len();
    let mut clips = Vec::new();
    let mut scene_seconds = Vec::new();

    for (i, (scene, voice_path)) in script.scenes.iter().zip(voices).enumerate() {
        let voice_len = voice::duration(voice_path).unwrap_or(0.0);
        let dur = scene.seconds.max(voice_len + 0.35);
        scene_seconds.push((dur * 100.0).round() / 100.0);
        let (inputs, vf) = scene_graph(&scene.on_screen, i, n, dur, settings, work)?;
        let clip = work.join(format!("clip-{i:02}.mp4"));
        let mut args: Vec<String> = vec!["-y".into(), "-loglevel".into(), "error".into()];
        args.extend(inputs);
        args.extend([
            "-i".into(), voice_path.display().to_string(),
            "-vf".into(), vf,
            "-af".into(), "apad,aresample=44100".into(),
            "-map".into(), "0:v".into(), "-map".into(), "1:a".into(),
            "-t".into(), format!("{dur:.2}"),
            "-c:v".into(), "libx264".into(), "-preset".into(), "veryfast".into(), "-crf".into(), "20".into(),
            "-r".into(), settings.fps.to_string(),
            "-c:a".into(), "aac".into(), "-ac".into(), "2".into(), "-ar".into(), "44100".into(),
            clip.display().to_string(),
        ]);
        ffmpeg(&args)?;
        clips.push(clip);
    }

    let list = work.join("concat.txt");
    std::fs::write(&list, clips.iter().map(|c| format!("file '{}'\n", clean(c))).collect::<String>())?;
    if let Some(dir) = out.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let voiced = if music.is_some() { work.join("voiced.mp4") } else { out.to_path_buf() };
    ffmpeg(&[
        "-y".into(), "-loglevel".into(), "error".into(),
        "-f".into(), "concat".into(), "-safe".into(), "0".into(),
        "-i".into(), list.display().to_string(),
        "-c".into(), "copy".into(), "-movflags".into(), "+faststart".into(),
        voiced.display().to_string(),
    ])?;

    if let Some((track, m)) = music {
        let duck = if m.ducking {
            "[mus][v1]sidechaincompress=threshold=0.03:ratio=10:attack=20:release=400[bed];[v2][bed]"
        } else {
            "[v2][mus]"
        };
        let graph = format!(
            "[0:a]asplit=2[v1][v2];[1:a]aresample=44100,volume={:.1}dB[mus];{duck}amix=inputs=2:duration=first:normalize=0,afade=t=out:st=0:d=0[a]",
            m.volume_db.clamp(-40.0, 0.0)
        );
        // Fade the bed out over the last 1.5 s.
        let total = voice::duration(&voiced).unwrap_or(0.0);
        let graph = graph.replace("afade=t=out:st=0:d=0", &format!("afade=t=out:st={:.2}:d=1.5", (total - 1.5).max(0.0)));
        ffmpeg(&[
            "-y".into(), "-loglevel".into(), "error".into(),
            "-i".into(), voiced.display().to_string(),
            "-stream_loop".into(), "-1".into(), "-i".into(), track.file.clone(),
            "-filter_complex".into(), graph,
            "-map".into(), "0:v".into(), "-map".into(), "[a]".into(),
            "-c:v".into(), "copy".into(), "-c:a".into(), "aac".into(), "-b:a".into(), "192k".into(),
            "-shortest".into(), "-movflags".into(), "+faststart".into(),
            out.display().to_string(),
        ])?;
    }
    Ok(Rendered { file: out.to_path_buf(), scene_seconds })
}

/// A finished render and the real length of each scene.
#[derive(Debug, Clone)]
pub struct Rendered {
    pub file: PathBuf,
    pub scene_seconds: Vec<f32>,
}

#[cfg(test)]
mod tests {
    use super::wrap;

    #[test]
    fn wraps_on_word_boundaries() {
        assert_eq!(wrap("Give it a role and an audience", 16), "Give it a role\nand an audience");
        assert_eq!(wrap("short", 16), "short");
    }
}
