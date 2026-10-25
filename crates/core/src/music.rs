//! Background music: the music view of the asset library (see `assets.rs`).
//! Only tracks with license information are used, and the credit line goes
//! into the description automatically.

use crate::assets::{self, Asset, AssetKind};
use crate::Result;
use std::path::Path;

/// A licensed music track. Same shape as any asset.
pub type Track = Asset;

impl Track {
    /// Attribution line for descriptions, e.g. `Music: "Calm" by X (CC0) https://…`.
    pub fn music_credit(&self) -> String {
        self.credit(AssetKind::Music)
    }
}

/// Lists every licensed track in `folder`. Files without license info are skipped.
pub fn scan(folder: &Path) -> Result<Vec<Track>> {
    assets::scan_folder(folder, AssetKind::Music)
}

/// Picks deterministically from `seed` (the video id), so re-renders keep the same music.
pub fn pick<'a>(tracks: &'a [Track], seed: &str) -> Option<&'a Track> {
    assets::pick(tracks, seed)
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
        assert_eq!(t[0].music_credit(), "Music: \"Calm\" by X (CC0) https://e.x");
        assert_eq!(pick(&t, "v1"), pick(&t, "v1"));
    }
}
