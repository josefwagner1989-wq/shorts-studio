//! Local 9:16 rendering with ffmpeg: one clip per scene (background, wrapped
//! on-screen text, progress bar, voiceover), then a lossless concat.

use crate::config::RenderSettings;
use crate::model::Script;
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

/// Greedy word wrap so on-screen text stays inside the safe area.
pub fn wrap(text: &str, max_chars: usize) -> String {
    let mut lines: Vec<String> = Vec::new();
    let mut cur = String::new();
    for w in text.split_whitespace() {
        if !cur.is_empty() && cur.chars().count() + 1 + w.chars().count() > max_chars {
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
        let tail: String = err.lines().rev().take(6).collect::<Vec<_>>().into_iter().rev().collect::<Vec<_>>().join("\n");
        return Err(Error::Tool { tool: "ffmpeg".into(), message: tail });
    }
    Ok(())
}

/// Renders `script` with one voice file per scene into `out` (MP4, H.264 + AAC).
/// Each scene lasts at least its scripted seconds and at least as long as its voiceover.
pub fn render_short(script: &Script, voices: &[PathBuf], settings: &RenderSettings, work: &Path, out: &Path) -> Result<PathBuf> {
    if voices.len() != script.scenes.len() {
        return Err(Error::Config("one voice file per scene is required".into()));
    }
    std::fs::create_dir_all(work)?;
    let font = find_font(settings);
    let n = script.scenes.len();
    let (w, h) = (settings.width, settings.height);
    let fontsize = (w as f32 * 0.085) as u32;
    let mut clips = Vec::new();

    for (i, (scene, voice_path)) in script.scenes.iter().zip(voices).enumerate() {
        let voice_len = voice::duration(voice_path).unwrap_or(0.0);
        let dur = scene.seconds.max(voice_len + 0.35);
        let text_file = work.join(format!("text-{i:02}.txt"));
        std::fs::write(&text_file, wrap(&scene.on_screen, 16))?;
        let font_arg = match &font {
            Some(f) => format!("fontfile='{}'", f.replace('\'', "")),
            None => "font='Sans'".to_string(),
        };
        let bar_w = (w as f32 * (i + 1) as f32 / n as f32) as u32;
        let vf = format!(
            "drawbox=x=0:y={by}:w={bar_w}:h=14:color={accent}:t=fill,\
drawtext={font_arg}:textfile='{tf}':fontsize={fontsize}:fontcolor={fg}:line_spacing={ls}:\
x=(w-text_w)/2:y=(h-text_h)/2",
            by = h - 14,
            accent = settings.accent,
            tf = text_file.display().to_string().replace('\'', ""),
            fg = settings.text_color,
            ls = fontsize / 4,
        );
        let clip = work.join(format!("clip-{i:02}.mp4"));
        ffmpeg(&[
            "-y".into(), "-loglevel".into(), "error".into(),
            "-f".into(), "lavfi".into(),
            "-i".into(), format!("color=c={}:s={w}x{h}:r={}:d={dur:.2}", settings.background, settings.fps),
            "-i".into(), voice_path.display().to_string(),
            "-vf".into(), vf,
            "-af".into(), "apad,aresample=44100".into(),
            "-t".into(), format!("{dur:.2}"),
            "-c:v".into(), "libx264".into(), "-preset".into(), "veryfast".into(), "-crf".into(), "20".into(),
            "-pix_fmt".into(), "yuv420p".into(),
            "-c:a".into(), "aac".into(), "-ac".into(), "2".into(), "-ar".into(), "44100".into(),
            clip.display().to_string(),
        ])?;
        clips.push(clip);
    }

    let list = work.join("concat.txt");
    std::fs::write(
        &list,
        clips.iter().map(|c| format!("file '{}'\n", c.display().to_string().replace('\'', ""))).collect::<String>(),
    )?;
    if let Some(dir) = out.parent() {
        std::fs::create_dir_all(dir)?;
    }
    ffmpeg(&[
        "-y".into(), "-loglevel".into(), "error".into(),
        "-f".into(), "concat".into(), "-safe".into(), "0".into(),
        "-i".into(), list.display().to_string(),
        "-c".into(), "copy".into(), "-movflags".into(), "+faststart".into(),
        out.display().to_string(),
    ])?;
    Ok(out.to_path_buf())
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
