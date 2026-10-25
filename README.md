# Shorts Studio

A Linux desktop app (Rust, GTK4 + libadwaita, Flatpak) that turns a channel strategy into finished vertical shorts:

**plan ideas → write script → critic review (auto-rewrite if weak) → voiceover → local 9:16 render → approve → publish**

Each thinking step can run on a different model. You pick the order per job; when one provider fails or reaches the daily limit you set, the next one answers.

| Job | Default order |
|---|---|
| Strategist (ideas) | Claude → Gemini → local |
| Writer (scripts) | GPT → Claude → local |
| Critic (review) | Gemini → Claude → local |

## Status: beta target Friday 30 October 2026

See [ROADMAP.md](ROADMAP.md). CI builds everything and runs an offline end-to-end selftest on every push. The selftest renders a real MP4 with mock models, and the MP4 is uploaded as a build artifact.

## Layout

```
crates/core   pipeline: providers, router, agents, voice, render, publish, store
crates/cli    `shorts` command: selftest, plan, run, approve, connect-youtube
crates/app    GTK4/libadwaita app with board, video dialog and Preferences
flatpak/      manifest, desktop file, metainfo, icon
```

## Setup

1. Open **Preferences → Models** and paste the API keys you have (Anthropic, OpenAI, Gemini), or switch on the local Ollama provider. Set a **daily token limit** per provider if you want to cap spend.
2. **Preferences → Channel**: describe the niche, audience, tone and what makes the channel original. Every prompt uses this.
3. **Preferences → Voice**: espeak works out of the box. For a natural voice, install a [Piper](https://github.com/rhasspy/piper) voice and point to its `.onnx` file, or use OpenAI TTS.
4. **Preferences → Publishing**: Outbox (folder) is the default. For YouTube, create a Desktop OAuth client in Google Cloud with the YouTube Data API enabled, paste the ID and secret, then press **Connect YouTube account**. Uploads start as **private** until you change visibility.

Keys are stored in `secrets.json` in the app's config directory with mode 0600, never in `settings.json`. Environment variables (`ANTHROPIC_API_KEY`, `OPENAI_API_KEY`, `GEMINI_API_KEY`) also work.

## Build

```sh
# core + CLI (any Linux with ffmpeg)
cargo build && cargo test
cargo run -p shorts-cli -- selftest /tmp/selftest   # offline end-to-end

# desktop app (needs libgtk-4-dev, libadwaita-1-dev)
cargo run -p shorts-studio

# Flatpak
flatpak-builder --user --install --force-clean build flatpak/io.github.shortsstudio.ShortsStudio.yml
flatpak run io.github.shortsstudio.ShortsStudio
```

## Platform rules this app follows

- One account and one key per provider, inside each provider's own limits.
- Videos uploaded to YouTube set `containsSyntheticMedia`, so YouTube shows its AI-content label.
- The critic flags generic or template-like scripts. YouTube does not monetize mass-produced, repetitive content, so keep human review on until your scores are consistently high.
- TikTok's Content Posting API only allows public posts after TikTok audits your app. Until then, use the Outbox and post manually.
