# Build prompt: Creator Studio

Paste this whole file as the first message of a coding session, or keep it in the repo and say "follow docs/VIBE_PROMPT.md". It describes the product, the architecture, the order of work and how to verify each step. It overrides older notes when they conflict.

---

## 1. What you are building

**Creator Studio** (working name; the repo and app id still say Shorts Studio) is a Linux desktop app for one creator or a small team. It turns a channel or brand strategy into finished, publishable media. Everything renders on the user's own machine. AI models only think and write; they never hold the user's files.

Five workspaces share one project library, one model router and one asset library:

| Workspace | Input | Output |
|---|---|---|
| **Strategy** | Goals, audience, niche, competitors, past results | Positioning, content pillars, a dated content calendar, per-piece briefs |
| **Shorts** | A brief or idea | 9:16 MP4, 15–60 s, voiceover, captions, music, description |
| **Long video** | A brief or outline | 16:9 MP4, 5–20 min, chapters, B-roll images, voiceover, music, description with chapters and sources |
| **Presentations** | A brief, outline or document | 16:9 slide deck exported as PDF and PNG per slide, optional narrated MP4 |
| **Design** | A brief plus brand kit | Thumbnails (1280×720), social posts (1080×1350, 1080×1080), banners (2560×1440), exported PNG/PDF |

A strategy item becomes a brief, and the brief opens in the matching workspace. Results flow back into Strategy as published items with stats.

## 2. Non-negotiables

1. **Official APIs only.** One account and one key per provider, used within that provider's limits. Never create extra accounts, rotate accounts or bypass rate limits. The router enforces the daily token limits the user sets.
2. **Human approval by default.** Nothing publishes without a click, unless the user turns on auto-publish *and* the critic score meets the threshold *and* the legal check has no high-severity finding.
3. **Legal and critical check before publishing.** Copyright, trademarks, defamation, health/finance/legal claims, privacy, music license, missing sources, AI disclosure, platform policy. Show it as a screening, never as legal advice.
4. **Disclosure.** YouTube uploads set `containsSyntheticMedia`. Every description ends with an AI-assistance line.
5. **Licensed assets only.** Music, images and fonts are used only with license metadata. Credits are written into descriptions and slide/design footers automatically.
6. **Secrets never in the repo or settings.json.** Keys live in `secrets.json` (0600) until the Secret Service portal (oo7) replaces it.
7. **Codecs bundled.** The Flatpak ships its own ffmpeg (H.264 via x264, AAC, freetype, fontconfig). Outside Flatpak, `render::check_ffmpeg()` explains anything that is missing.

## 3. Stack and constraints

- Rust workspace: `crates/core` (no UI), `crates/cli` (`shorts` command), `crates/app` (GTK4 0.9 + libadwaita 0.7, features `v4_12` / `v1_5`).
- Flatpak on the GNOME 47 runtime: manifest in `flatpak/`, installer bundle built by CI, `scripts/install.sh`.
- Video: ffmpeg CLI driven from `core::render`. Slides and designs: generate **SVG from templates**, rasterize with **resvg**, and write PDF with **svg2pdf**. All three are pure Rust, so no extra system libraries are needed.
- Models: `core::llm` (Anthropic, OpenAI, Gemini, Ollama, Mock) behind `core::router` (per-role preference order, fallback, daily budgets).
- Voice: espeak-ng (bundled), Piper (local neural), OpenAI TTS.
- **The coding sandbox cannot reach crates.io.** You cannot compile locally. Push to a branch or main, and GitHub Actions is the compiler and test runner. Read results with unauthenticated `curl` against `api.github.com` (runs → jobs → `check-runs/<job_id>/annotations`). `ci/run.sh` turns build/test errors into annotations; keep using it for every new CI step.
- Keep dependencies few and well known. Every new crate needs one sentence in the commit message on why.

## 4. Architecture (extend, don't rewrite)

```
core/
  config.rs     Settings: providers, roles, voice, render style, music, publish, strategy
  secrets.rs    API keys (0600)
  llm.rs        provider clients + MockProvider (must answer every TASK: prompt)
  router.rs     role → provider order, fallback, budgets, trace
  agents.rs     prompts: ideas, script, critic, legal   ← add: strategy, outline, slides, design copy
  model.rs      Video, Script, Scene, Critique, LegalReview, Source, description()
  music.rs      licensed track library, credits
  voice.rs      TTS engines, durations
  render.rs     9:16 scenes, style, music ducking, preview frame   ← generalize to Format (9:16, 16:9)
  publish.rs    Outbox, YouTube (upload, channel, stats)
  store.rs      JSON per item under the data dir
  workflow.rs   resumable step chain per item
```

Add, in this shape:

- `project.rs`: a `Project` (id, kind: Short | Long | Deck | Design | Plan, brief, status, links to source plan items) that generalizes `Video`. Migrate `Video` files on load and keep old JSON readable.
- `brand.rs`: brand kit with colours, fonts (files + license), logo, tone of voice, plus a default per-format style. Used by every renderer.
- `assets.rs`: one library for music, images and fonts with license metadata. `music.rs` becomes a view on it.
- `slides.rs`: deck model (slides with title, bullets, notes, image, layout id), SVG templates, export to PDF/PNG, and an optional narrated MP4 that reuses `voice` and `render`.
- `design.rs`: design model (format, layers: text, image, shape), SVG templates, export to PNG/PDF.
- `strategy.rs`: plan model (pillars, calendar entries with date, format, brief, status), agents for planning and review, and a stats import from YouTube.
- `captions.rs`: word-timed captions (estimate from TTS duration; refine with Whisper later), burned into video and exported as SRT.

The app gets an `adw::NavigationSplitView` with sidebar workspaces (Strategy, Shorts, Long video, Presentations, Design, Assets, Settings). Each workspace has a list or board, a detail view and one primary action.

## 5. Order of work (each milestone ends green in CI)

**M0 — Beta of what exists** (due Fri 30 Oct 2026; see ROADMAP.md)
Shorts end to end, Preferences, style editor, music, sources, legal check, YouTube link, and the Flatpak installer as a release asset.

**M1 — Foundation for many formats**
`Project` model + migration, `Format { width, height, safe_area }`, `brand.rs`, `assets.rs`, sidebar navigation. Done when: a Short still renders identically (selftest), and old video JSON loads.

**M2 — Long video**
Outline agent (chapters), per-chapter script, 16:9 render, image B-roll from the asset library with Ken Burns pan/zoom, chapter timestamps in the description, captions + SRT. Done when: the selftest renders a 3-chapter, ≥60 s 16:9 MP4 with mock models and the description contains 00:00 chapters.

**M3 — Presentations**
Deck agent (outline → slides with notes), 5 SVG layouts (title, bullets, image+text, quote, two-column), brand kit applied, PDF + PNG export, narrated MP4 option. Done when: CI exports a 6-slide PDF and PNGs from a mock brief, and a snapshot test checks the SVG structure.

**M4 — Design**
Thumbnail, post and banner templates; agent writes headline variants; three variants per brief; export PNG/PDF; the critic scores readability at small size. Done when: CI exports a 1280×720 thumbnail and a 1080×1350 post from a mock brief.

**M5 — Strategy**
Plan agent (pillars, 4-week calendar), calendar view, brief → project with one click, weekly review agent using YouTube stats (views, likes per video). Done when: a mock plan creates projects of three different kinds and the review reads mocked stats.

**M6 — Polish for 1.0**
Secret Service portal, offline Flathub build (cargo-sources.json), accessibility pass, translations (de, en), crash-safe autosave, user docs.

## 6. How to work in every session

1. Read ROADMAP.md and this file. Check the latest CI run. **If CI is red, fixing it comes before anything else.**
2. Pick the next unchecked item. State in one line what "done" means for it before writing code.
3. Make the smallest change that reaches "done". Add or extend tests:
   - unit tests in `core` for pure logic (parsing, timing, routing, templates);
   - the Mock provider answers every new `TASK:` prompt with valid JSON;
   - `shorts selftest` (or a new `selftest-<kind>`) runs the new path end to end offline and prints one JSON line with `"ok": true`;
   - an export check in CI: ffprobe for video length/resolution, `pdfinfo` page count for PDFs, image size for PNGs.
4. Push, wait for CI, read the annotations, fix. Repeat until green. Do not stack new work on a red build.
5. Tick the item in ROADMAP.md, then commit with a message that says what changed and why.
6. Report in at most 10 lines: CI link, what was tested and the result, commits, the next item, blockers for the owner.

## 7. Quality bar

- No `unwrap()` on user data, files or network results in `core`. Errors carry a sentence the user can act on ("Add an API key for gemini in Preferences").
- Long work runs off the UI thread (`gio::spawn_blocking`). The UI shows which step is running and never freezes.
- Every step is resumable: state is saved after each step, so a crash or failed provider continues where it stopped.
- Renders are deterministic for the same inputs (seeded choices, e.g. music pick by project id).
- Text in UI and errors is plain and specific. No jargon like "payload", "pipeline failure" or "invalid state".
- Clippy clean is the goal; never silence a warning without a comment saying why.

## 8. Out of scope until asked

Other platforms besides YouTube, stock footage downloads, generative video models, multi-user sync, a web version, telemetry.

## 9. Open decisions for the owner

- Final product name and app id (rename before 1.0, ideally in M1).
- Whether to bundle Piper and one default voice in the Flatpak (adds ~60 MB).
- Default language of generated content (German, English, or per project).
