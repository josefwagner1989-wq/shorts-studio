//! Brand kit: one place for the colours, fonts, logo and voice that every
//! output uses. Empty fields mean "no opinion", so the style editor's values stay.

use crate::config::RenderSettings;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Brand {
    pub name: String,
    /// Colours as ffmpeg/SVG colours: `0xRRGGBB` or `#RRGGBB` or a name.
    pub background: String,
    pub text: String,
    pub accent: String,
    /// Font files (.ttf/.otf) and the license that allows embedding them.
    pub heading_font: String,
    pub body_font: String,
    pub font_license: String,
    /// Logo image (PNG/SVG). Used by designs and slides.
    pub logo: String,
    /// How the brand talks, e.g. "calm, precise, a little dry humour, no hype".
    pub tone: String,
}

/// `#RRGGBB` → `0xRRGGBB` so the same value works in ffmpeg filters.
pub fn ffmpeg_color(c: &str) -> String {
    match c.strip_prefix('#') {
        Some(hex) if hex.len() == 6 && hex.chars().all(|ch| ch.is_ascii_hexdigit()) => format!("0x{hex}"),
        _ => c.to_string(),
    }
}

impl Brand {
    pub fn is_empty(&self) -> bool {
        *self == Brand::default()
    }

    /// Overrides style values with the brand's, field by field, where the brand sets one.
    pub fn apply(&self, style: &RenderSettings) -> RenderSettings {
        let mut s = style.clone();
        let set = |target: &mut String, v: &str, color: bool| {
            if !v.trim().is_empty() {
                *target = if color { ffmpeg_color(v.trim()) } else { v.trim().to_string() };
            }
        };
        set(&mut s.background, &self.background, true);
        set(&mut s.text_color, &self.text, true);
        set(&mut s.accent, &self.accent, true);
        set(&mut s.font_file, &self.heading_font, false);
        s
    }

    /// Lines added to every model prompt so output sounds like the brand.
    pub fn as_prompt(&self) -> String {
        let mut out = String::new();
        if !self.name.trim().is_empty() {
            out.push_str(&format!("brand: {}\n", self.name.trim()));
        }
        if !self.tone.trim().is_empty() {
            out.push_str(&format!("brand voice: {}\n", self.tone.trim()));
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_brand_changes_nothing() {
        let style = RenderSettings::default();
        assert_eq!(Brand::default().apply(&style), style);
        assert!(Brand::default().as_prompt().is_empty());
    }

    #[test]
    fn brand_overrides_only_what_it_sets() {
        let b = Brand { accent: "#FF6600".into(), tone: "calm".into(), ..Default::default() };
        let s = b.apply(&RenderSettings::default());
        assert_eq!(s.accent, "0xFF6600");
        assert_eq!(s.background, RenderSettings::default().background);
        assert_eq!(b.as_prompt(), "brand voice: calm\n");
        assert_eq!(ffmpeg_color("white"), "white");
    }
}
