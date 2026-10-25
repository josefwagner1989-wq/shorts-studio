# Roadmap to the beta (Friday 30 October 2026)

A daily check runs at 11:30 (Europe/Berlin). It reads CI, fixes what broke, takes the next unchecked item, pushes, and reports by 12:00.

## Beta definition

Someone installs the Flatpak, adds one API key in Preferences and presses **Plan ideas → Produce all**. They get a playable 9:16 MP4 with a voiceover, and it can be published to the Outbox or to YouTube (private).

## Checklist

### Mon 26 Oct
- [ ] CI green: core, CLI and GTK app compile, unit tests pass
- [ ] Offline selftest renders a playable MP4 in CI (artifact + frame.png)
- [ ] Commit the CI-generated `Cargo.lock`

### Tue 27 Oct
- [ ] Unit tests for `store`, `secrets` (0600 permissions, masking), `workflow` resume after a failed step
- [ ] Live smoke test per provider behind an opt-in CI secret (skipped when no key)
- [ ] Captions: burn word-timed subtitles from the voiceover text

### Wed 28 Oct
- [ ] Flatpak job green and the `.flatpak` bundle downloadable from CI
- [ ] Piper voice inside the Flatpak (bundle piper, pick a voice file in Preferences)
- [ ] Progress shown in the app while producing (step label in the video dialog)

### Thu 29 Oct
- [ ] YouTube upload tested end to end on a private video
- [ ] Usage page: tokens used today per provider against the limit
- [ ] Error states in the app: missing key, Ollama not running, ffmpeg failure

### Fri 30 Oct — beta
- [ ] Fresh-install test from the Flatpak bundle
- [ ] Tag `v0.1.0-beta` with release notes

## After the beta
- Secret Service portal (oo7) instead of `secrets.json`
- Offline Flathub build (cargo-sources.json)
- Background music and B-roll images
