# Roadmap

The full product spec, architecture and way of working are in [docs/VIBE_PROMPT.md](docs/VIBE_PROMPT.md). This file tracks progress.

## M0 — Beta (Friday 30 October 2026)

A daily check runs at 11:30 (Europe/Berlin). It reads CI, fixes what broke, takes the next unchecked item, pushes, and reports by 12:00.

## Beta definition

Someone installs the Flatpak, adds one API key in Preferences and presses **Plan ideas → Produce all**. They get a playable 9:16 MP4 with a voiceover, and it can be published to the Outbox or to YouTube (private).

## Checklist

### Mon 26 Oct
- [x] CI green: core, CLI and GTK app compile, unit tests pass
- [x] Offline selftest renders a playable MP4 in CI (21 s, voice + music + legal check)
- [ ] Commit the CI-generated `Cargo.lock`

### Sun 25 Oct (done today)
- [x] Style editor with live preview frame
- [x] Royalty-free music library with ducking and automatic credits
- [x] Sources per video, listed in the description
- [x] Legal & critical check button; blocks auto-publish on high risk
- [x] YouTube channel shown after sign-in; stats API ready
- [x] Flatpak installer bundle + release job + install.sh

### Tue 27 Oct
- [ ] Unit tests for `store`, `secrets` (0600 permissions, masking), `workflow` resume after a failed step
- [ ] Live smoke test per provider behind an opt-in CI secret (skipped when no key)
- [ ] Captions: burn word-timed subtitles from the voiceover text

### Wed 28 Oct
- [x] Flatpak job green and the `.flatpak` bundle downloadable from CI (21 MB, bundles ffmpeg/x264/espeak-ng)
- [ ] Piper voice inside the Flatpak (bundle piper, pick a voice file in Preferences)
- [ ] Progress shown in the app while producing (step label in the video dialog)

### Thu 29 Oct
- [ ] YouTube upload tested end to end on a private video
- [ ] Usage page: tokens used today per provider against the limit
- [ ] Error states in the app: missing key, Ollama not running, ffmpeg failure

### Fri 30 Oct — beta
- [ ] Fresh-install test from the Flatpak bundle
- [ ] Tag `v0.1.0-beta` with release notes

## After the beta (details in docs/VIBE_PROMPT.md §5)
- [x] M1 Foundation: project kinds + formats, brand kit, asset library, sidebar navigation
- [ ] M2 Long video: ~~outline → chapters → 16:9 render → YouTube chapters~~ ✓, ~~captions + SRT~~ ✓, ~~Ken Burns B-roll with credits~~ ✓, workspace UI
- [ ] M3 Presentations: SVG slides → PDF/PNG, narrated MP4
- [ ] M4 Design: thumbnails, posts, banners
- [ ] M5 Strategy: plan, calendar, briefs → projects, weekly review with YouTube stats
- [ ] M6 1.0 polish: Secret Service portal, offline Flathub build, translations, docs
