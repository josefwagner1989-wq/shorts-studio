//! Asset library: music, images and fonts the user is allowed to use.
//!
//! Layout: `<library>/music`, `<library>/images`, `<library>/fonts`. Every file
//! needs license information, or it is never used. Give it a sidecar
//! `<file>.json` with `{"title", "artist", "license", "url"}`, or an entry in a
//! `library.json` in its folder: `[{"file", "title", "artist", "license", "url"}, ...]`.
//! Credits are generated from this metadata.

use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum AssetKind {
    Music,
    Image,
    Font,
}

impl AssetKind {
    pub fn folder(self) -> &'static str {
        match self {
            AssetKind::Music => "music",
            AssetKind::Image => "images",
            AssetKind::Font => "fonts",
        }
    }

    fn extensions(self) -> &'static [&'static str] {
        match self {
            AssetKind::Music => &["mp3", "ogg", "opus", "m4a", "flac", "wav", "aac"],
            AssetKind::Image => &["png", "jpg", "jpeg", "webp", "svg"],
            AssetKind::Font => &["ttf", "otf"],
        }
    }

    fn noun(self) -> &'static str {
        match self {
            AssetKind::Music => "Music",
            AssetKind::Image => "Image",
            AssetKind::Font => "Font",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Asset {
    /// File name in `library.json`; the full path once scanned.
    #[serde(default)]
    pub file: String,
    pub title: String,
    #[serde(default)]
    pub artist: String,
    /// e.g. "CC0", "CC BY 4.0", "SIL OFL 1.1", "Pixabay Content License".
    pub license: String,
    #[serde(default)]
    pub url: String,
}

impl Asset {
    /// Attribution line, e.g. `Music: "Calm" by X (CC0) https://…`.
    pub fn credit(&self, kind: AssetKind) -> String {
        let by = if self.artist.is_empty() { String::new() } else { format!(" by {}", self.artist) };
        let url = if self.url.is_empty() { String::new() } else { format!(" {}", self.url) };
        format!("{}: \"{}\"{by} ({}){url}", kind.noun(), self.title, self.license)
    }
}

/// Lists the licensed files of one kind directly inside `folder`.
pub fn scan_folder(folder: &Path, kind: AssetKind) -> Result<Vec<Asset>> {
    if !folder.is_dir() {
        return Err(Error::Config(format!("{} folder not found: {}", kind.folder(), folder.display())));
    }
    let library: Vec<Asset> = std::fs::read_to_string(folder.join("library.json"))
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default();
    let mut files: Vec<PathBuf> = std::fs::read_dir(folder)?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            p.extension()
                .and_then(|x| x.to_str())
                .is_some_and(|x| kind.extensions().contains(&x.to_lowercase().as_str()))
        })
        .collect();
    files.sort();
    let mut out = Vec::new();
    for path in files {
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or_default().to_string();
        let sidecar = path.with_file_name(format!("{name}.json"));
        let meta: Option<Asset> = std::fs::read_to_string(&sidecar)
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .or_else(|| library.iter().find(|a| a.file == name).cloned());
        if let Some(mut a) = meta {
            if a.license.trim().is_empty() || a.title.trim().is_empty() {
                continue;
            }
            a.file = path.display().to_string();
            out.push(a);
        }
    }
    Ok(out)
}

pub struct Library {
    pub root: PathBuf,
}

impl Library {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn folder(&self, kind: AssetKind) -> PathBuf {
        self.root.join(kind.folder())
    }

    /// Creates the three folders so the user knows where to put files.
    pub fn ensure(&self) -> Result<()> {
        for k in [AssetKind::Music, AssetKind::Image, AssetKind::Font] {
            std::fs::create_dir_all(self.folder(k))?;
        }
        Ok(())
    }

    /// Licensed assets of one kind; an empty list if the folder does not exist yet.
    pub fn list(&self, kind: AssetKind) -> Result<Vec<Asset>> {
        let f = self.folder(kind);
        if !f.exists() {
            return Ok(Vec::new());
        }
        scan_folder(&f, kind)
    }
}

/// Picks deterministically from `seed`, so re-renders keep the same choice.
pub fn pick<'a>(assets: &'a [Asset], seed: &str) -> Option<&'a Asset> {
    if assets.is_empty() {
        return None;
    }
    let h = seed.bytes().fold(1469598103934665603u64, |h, b| (h ^ b as u64).wrapping_mul(1099511628211));
    assets.get((h % assets.len() as u64) as usize)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn library_lists_only_licensed_files_per_kind() {
        let dir = tempfile::tempdir().unwrap();
        let lib = Library::new(dir.path());
        lib.ensure().unwrap();
        let img = lib.folder(AssetKind::Image);
        std::fs::write(img.join("bg.png"), b"x").unwrap();
        std::fs::write(img.join("bg.png.json"), r#"{"title":"Desk","artist":"Ann","license":"Pixabay Content License","url":"https://pixabay.com/x"}"#).unwrap();
        std::fs::write(img.join("nolicense.jpg"), b"x").unwrap();
        let fonts = lib.folder(AssetKind::Font);
        std::fs::write(fonts.join("Inter.ttf"), b"x").unwrap();
        std::fs::write(fonts.join("library.json"), r#"[{"file":"Inter.ttf","title":"Inter","license":"SIL OFL 1.1"}]"#).unwrap();
        std::fs::write(fonts.join("readme.txt"), b"not a font").unwrap();

        let images = lib.list(AssetKind::Image).unwrap();
        assert_eq!(images.len(), 1);
        assert_eq!(images[0].credit(AssetKind::Image), "Image: \"Desk\" by Ann (Pixabay Content License) https://pixabay.com/x");
        assert_eq!(lib.list(AssetKind::Font).unwrap().len(), 1);
        assert!(lib.list(AssetKind::Music).unwrap().is_empty());
        assert!(Library::new(dir.path().join("missing")).list(AssetKind::Music).unwrap().is_empty());
    }
}
