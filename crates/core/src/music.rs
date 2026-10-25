//! Royalty-free music library.
//!
//! Put audio files (mp3, ogg, opus, m4a, flac, wav) in one folder. Each track
//! needs license information, or it is never used. Give it either a sidecar
//! `<file>.json` with `{"title", "artist", "license", "url"}`, or an entry in one
//! `library.json` in the folder: `[{"file", "title", "artist", "license", "url"}, ...]`.
//! Typical sources: YouTube Audio Library, Pixabay Music, Free Music Archive (CC0 / CC BY).
//! The credit line goes into the video description automatically.

use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

const AUDIO: &[&str] = &["mp3", "ogg", "opus", "m4a", "flac", "wav", "aac"];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Track {
    #[serde(default)]
    pub file: String,
    pub title: String,
    #[serde(default)]
    pub artist: String,
    /// e.g. "CC0", "CC BY 4.0", "YouTube Audio Library (no attribution required)".
    pub license: String,
    #[serde(default)]
    pub url: String,
}

impl Track {
    /// Attribution line for descriptions.
    pub fn credit(&self) -> String {
        let by = if self.artist.is_empty() { String::new() } else { format!(" by {}", self.artist) };
        let url = if self.url.is_empty() { String::new() } else { format!(" {}", self.url) };
        format!("Music: \"{}\"{by} ({}){url}", self.title, self.license)
    }
}

/// Lists every track that has license information. Files without it are skipped.
pub fn scan(folder: &Path) -> Result<Vec<Track>> {
    if !folder.is_dir() {
        return Err(Error::Config(format!("music folder not found: {}", folder.display())));
    }
    let library: Vec<Track> = std::fs::read_to_string(folder.join("library.json"))
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default();
    let mut out = Vec::new();
    let mut files: Vec<PathBuf> = std::fs::read_dir(folder)?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().and_then(|x| x.to_str()).is_some_and(|x| AUDIO.contains(&x.to_lowercase().as_str())))
        .collect();
    files.sort();
    for path in files {
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or_default().to_string();
        let sidecar = path.with_extension(format!("{}.json", path.extension().and_then(|x| x.to_str()).unwrap_or("")));
        let meta: Option<Track> = std::fs::read_to_string(&sidecar)
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .or_else(|| library.iter().find(|t| t.file == name).cloned());
        if let Some(mut t) = meta {
            if t.license.trim().is_empty() || t.title.trim().is_empty() {
                continue;
            }
            t.file = path.display().to_string();
            out.push(t);
        }
    }
    Ok(out)
}

/// Picks a track deterministically from `seed` (the video id), so re-renders keep the same music.
pub fn pick<'a>(tracks: &'a [Track], seed: &str) -> Option<&'a Track> {
    if tracks.is_empty() {
        return None;
    }
    let h = seed.bytes().fold(1469598103934665603u64, |h, b| (h ^ b as u64).wrapping_mul(1099511628211));
    tracks.get((h % tracks.len() as u64) as usize)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_licensed_tracks_are_used() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.mp3"), b"x").unwrap();
        std::fs::write(dir.path().join("a.mp3.json"), r#"{"title":"Calm","artist":"X","license":"CC0","url":"https://e.x"}"#).unwrap();
        std::fs::write(dir.path().join("b.mp3"), b"x").unwrap(); // no license → skipped
        std::fs::write(dir.path().join("c.ogg"), b"x").unwrap();
        std::fs::write(dir.path().join("library.json"), r#"[{"file":"c.ogg","title":"Drive","license":"CC BY 4.0"}]"#).unwrap();
        let t = scan(dir.path()).unwrap();
        assert_eq!(t.len(), 2);
        assert_eq!(t[0].credit(), "Music: \"Calm\" by X (CC0) https://e.x");
        assert_eq!(pick(&t, "v1"), pick(&t, "v1"));
    }
}
