//! Timed captions from the voiceover text.
//!
//! Each scene's voiceover is split into short chunks, and each chunk gets a
//! share of the scene's real voice duration proportional to its length in
//! characters (a good proxy for speaking time). The same cues are burned into
//! the video and written as an `.srt` file.

#[derive(Debug, Clone, PartialEq)]
pub struct Cue {
    pub start: f32,
    pub end: f32,
    pub text: String,
}

/// Splits `text` into chunks of at most `max_words` words, breaking early after
/// sentence punctuation so a caption never spans two sentences.
pub fn chunks(text: &str, max_words: usize) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur: Vec<&str> = Vec::new();
    for w in text.split_whitespace() {
        cur.push(w);
        let ends_sentence = w.ends_with(['.', '!', '?', ':', ';']);
        if cur.len() >= max_words.max(1) || ends_sentence {
            out.push(cur.join(" "));
            cur.clear();
        }
    }
    if !cur.is_empty() {
        out.push(cur.join(" "));
    }
    out
}

/// Cues for one scene, relative to the scene start. `speech` is the voice length in seconds.
pub fn scene_cues(voiceover: &str, speech: f32, max_words: usize) -> Vec<Cue> {
    let parts = chunks(voiceover, max_words);
    let total: usize = parts.iter().map(|p| p.chars().count().max(1)).sum();
    if parts.is_empty() || speech <= 0.0 {
        return Vec::new();
    }
    let mut t = 0.0;
    parts
        .into_iter()
        .map(|p| {
            let len = speech * p.chars().count().max(1) as f32 / total as f32;
            let cue = Cue { start: t, end: t + len, text: p };
            t += len;
            cue
        })
        .collect()
}

fn srt_time(t: f32) -> String {
    let ms = (t.max(0.0) * 1000.0).round() as u64;
    format!("{:02}:{:02}:{:02},{:03}", ms / 3_600_000, (ms / 60_000) % 60, (ms / 1000) % 60, ms % 1000)
}

/// SubRip text for absolute cues.
pub fn srt(cues: &[Cue]) -> String {
    cues.iter()
        .enumerate()
        .map(|(i, c)| format!("{}\n{} --> {}\n{}\n", i + 1, srt_time(c.start), srt_time(c.end), c.text))
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chunks_break_at_sentences_and_length() {
        assert_eq!(chunks("One two three four five six. Seven", 4), vec!["One two three four", "five six.", "Seven"]);
    }

    #[test]
    fn cues_fill_the_speech_time() {
        let c = scene_cues("Hello there. General idea", 3.0, 5);
        assert_eq!(c.len(), 2);
        assert_eq!(c[0].start, 0.0);
        assert!((c[1].end - 3.0).abs() < 1e-4);
        assert!(scene_cues("", 3.0, 5).is_empty());
    }

    #[test]
    fn srt_format() {
        let s = srt(&[Cue { start: 0.0, end: 1.5, text: "Hi".into() }, Cue { start: 61.25, end: 3725.0, text: "Bye".into() }]);
        assert_eq!(s, "1\n00:00:00,000 --> 00:00:01,500\nHi\n\n2\n00:01:01,250 --> 01:02:05,000\nBye\n");
    }
}
