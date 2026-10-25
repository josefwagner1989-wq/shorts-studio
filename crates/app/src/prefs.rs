//! Preferences: API keys and usage limits per provider, role routing, voice,
//! publishing and channel strategy. Saved when the dialog closes.

use crate::board::{refresh, spawn};
use crate::State;
use adw::prelude::*;
use gtk::glib;
use shorts_core::config::{ProviderKind, RenderSettings, VoiceEngine};
use shorts_core::publish::YouTube;
use shorts_core::secrets::Secrets;
use std::rc::Rc;

struct StyleRows {
    font_file: adw::EntryRow,
    font_scale: adw::SpinRow,
    position: adw::ComboRow,
    uppercase: adw::SwitchRow,
    wrap: adw::SpinRow,
    text_color: adw::EntryRow,
    text_box: adw::SwitchRow,
    box_color: adw::EntryRow,
    box_opacity: adw::SpinRow,
    background: adw::EntryRow,
    background_image: adw::EntryRow,
    dim: adw::SpinRow,
    accent: adw::EntryRow,
    progress: adw::SwitchRow,
}

const POSITIONS: [&str; 3] = ["top", "center", "bottom"];

impl StyleRows {
    fn read(&self, base: &RenderSettings) -> RenderSettings {
        let t = |e: &adw::EntryRow| e.text().trim().to_string();
        RenderSettings {
            font_file: t(&self.font_file),
            font_scale: self.font_scale.value() as f32,
            text_position: combo_value(&self.position, &POSITIONS),
            uppercase: self.uppercase.is_active(),
            wrap_chars: self.wrap.value() as usize,
            text_color: if t(&self.text_color).is_empty() { "white".into() } else { t(&self.text_color) },
            text_box: self.text_box.is_active(),
            text_box_color: if t(&self.box_color).is_empty() { "black".into() } else { t(&self.box_color) },
            text_box_opacity: self.box_opacity.value() as f32,
            background: if t(&self.background).is_empty() { "black".into() } else { t(&self.background) },
            background_image: t(&self.background_image),
            background_dim: self.dim.value() as f32,
            accent: if t(&self.accent).is_empty() { "white".into() } else { t(&self.accent) },
            progress_bar: self.progress.is_active(),
            ..base.clone()
        }
    }
}

fn spin(title: &str, min: f64, max: f64, step: f64, value: f64, digits: u32) -> adw::SpinRow {
    let r = adw::SpinRow::with_range(min, max, step);
    r.set_title(title);
    r.set_digits(digits);
    r.set_value(value);
    r
}

struct ProviderRows {
    enabled: adw::SwitchRow,
    key: Option<adw::PasswordEntryRow>,
    model: adw::EntryRow,
    budget: adw::SpinRow,
    base_url: adw::EntryRow,
}

fn entry(title: &str, text: &str) -> adw::EntryRow {
    adw::EntryRow::builder().title(title).text(text).build()
}

fn combo(title: &str, items: &[&str], current: &str) -> adw::ComboRow {
    let model = gtk::StringList::new(items);
    let selected = items.iter().position(|i| *i == current).unwrap_or(0) as u32;
    adw::ComboRow::builder().title(title).model(&model).selected(selected).build()
}

fn combo_value(row: &adw::ComboRow, items: &[&str]) -> String {
    items.get(row.selected() as usize).unwrap_or(&items[0]).to_string()
}

fn usage_today(state: &State) -> serde_json::Value {
    state.settings.borrow().data_path().ok()
        .and_then(|d| std::fs::read_to_string(d.join("usage.json")).ok())
        .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
        .map(|v| v["tokens"].clone())
        .unwrap_or(serde_json::Value::Null)
}

const VOICES: [&str; 4] = ["espeak", "piper", "openai", "none"];
const TARGETS: [&str; 2] = ["outbox", "youtube"];
const PRIVACY: [&str; 3] = ["private", "unlisted", "public"];

pub fn open(state: &Rc<State>) {
    let s = state.settings.borrow().clone();
    let secrets = Secrets::load();
    let usage = usage_today(state);
    let dialog = adw::PreferencesDialog::builder().title("Preferences").build();

    /* ---- Models ---- */
    let models = adw::PreferencesPage::builder().title("Models").icon_name("network-server-symbolic").build();
    let mut rows = Vec::new();
    for p in &s.providers {
        let kind = match p.kind {
            ProviderKind::Anthropic => "Anthropic · Claude",
            ProviderKind::OpenAi => "OpenAI · GPT",
            ProviderKind::Gemini => "Google · Gemini",
            ProviderKind::Ollama => "Ollama · local, free",
            ProviderKind::Mock => "Offline test provider",
        };
        let used = usage[&p.name].as_u64().unwrap_or(0);
        let budget_text = if p.daily_token_budget == 0 { "no limit".to_string() } else { format!("{} limit", p.daily_token_budget) };
        let group = adw::PreferencesGroup::builder()
            .title(glib::markup_escape_text(&p.name).as_str())
            .description(format!("{kind} — {used} tokens used today, {budget_text}"))
            .build();
        let enabled = adw::SwitchRow::builder().title("Use this provider").active(p.enabled).build();
        group.add(&enabled);
        let key = if matches!(p.kind, ProviderKind::Ollama | ProviderKind::Mock) {
            None
        } else {
            let title = match secrets.masked(&p.name) {
                Some(m) => format!("API key (saved {m})"),
                None if p.api_key_env.as_deref().and_then(|e| std::env::var(e).ok()).is_some() => "API key (from environment)".to_string(),
                None => "API key".to_string(),
            };
            let r = adw::PasswordEntryRow::builder().title(title.as_str()).build();
            group.add(&r);
            Some(r)
        };
        let model = entry("Model", &p.model);
        group.add(&model);
        let budget = adw::SpinRow::with_range(0.0, 50_000_000.0, 10_000.0);
        budget.set_title("Daily token limit (0 = no limit)");
        budget.set_value(p.daily_token_budget as f64);
        group.add(&budget);
        let base_url = entry("Server URL (optional)", p.base_url.as_deref().unwrap_or(""));
        group.add(&base_url);
        models.add(&group);
        rows.push(ProviderRows { enabled, key, model, budget, base_url });
    }

    /* ---- Routing ---- */
    let routing = adw::PreferencesPage::builder().title("Routing").icon_name("media-playlist-shuffle-symbolic").build();
    let names: Vec<String> = s.providers.iter().map(|p| p.name.clone()).collect();
    let rg = adw::PreferencesGroup::builder()
        .title("Which model does which job")
        .description(format!("Comma-separated, in order of preference. If one fails or hits its limit, the next one answers. Providers: {}", names.join(", ")))
        .build();
    let strat = entry("Strategist (plans ideas)", &s.roles.strategist.join(", "));
    let writer = entry("Writer (scripts)", &s.roles.writer.join(", "));
    let critic = entry("Critic (reviews scripts)", &s.roles.critic.join(", "));
    rg.add(&strat);
    rg.add(&writer);
    rg.add(&critic);
    routing.add(&rg);

    /* ---- Voice ---- */
    let voice_page = adw::PreferencesPage::builder().title("Voice").icon_name("audio-input-microphone-symbolic").build();
    let vg = adw::PreferencesGroup::builder()
        .title("Voiceover")
        .description("espeak: built in, robotic. piper: natural local voice, needs a .onnx voice file. openai: cloud voice, uses your OpenAI key. none: silent.")
        .build();
    let (engine_now, param_now, model_now) = match &s.voice {
        VoiceEngine::Espeak { voice } => ("espeak", voice.clone(), String::new()),
        VoiceEngine::Piper { model } => ("piper", model.clone(), String::new()),
        VoiceEngine::OpenAi { model, voice } => ("openai", voice.clone(), model.clone()),
        VoiceEngine::None => ("none", String::new(), String::new()),
    };
    let engine = combo("Engine", &VOICES, engine_now);
    let vparam = entry("Voice (espeak/openai) or voice file path (piper)", &param_now);
    let vmodel = entry("OpenAI TTS model", if model_now.is_empty() { "gpt-4o-mini-tts" } else { model_now.as_str() });
    vg.add(&engine);
    vg.add(&vparam);
    vg.add(&vmodel);
    voice_page.add(&vg);

    /* ---- Publishing ---- */
    let pub_page = adw::PreferencesPage::builder().title("Publishing").icon_name("send-to-symbolic").build();
    let pg = adw::PreferencesGroup::builder()
        .title("Where finished videos go")
        .description("Outbox saves the MP4 and caption to a folder. YouTube uploads through the official API and marks the video as AI-generated.")
        .build();
    let target = combo("Publish to", &TARGETS, &s.publish.target);
    let auto = adw::SwitchRow::builder().title("Publish automatically")
        .subtitle("Skip manual approval when the critic score is high enough").active(s.publish.auto_publish).build();
    let min_score = adw::SpinRow::with_range(1.0, 10.0, 1.0);
    min_score.set_title("Minimum critic score");
    min_score.set_value(s.publish.min_score as f64);
    let privacy = combo("YouTube visibility", &PRIVACY, &s.publish.youtube_privacy);
    pg.add(&target);
    pg.add(&auto);
    pg.add(&min_score);
    pg.add(&privacy);
    pub_page.add(&pg);

    let yg = adw::PreferencesGroup::builder()
        .title("YouTube account")
        .description("Create an OAuth client (Desktop app) in your Google Cloud project with the YouTube Data API enabled, then paste its ID and secret.")
        .build();
    let yt_id = adw::PasswordEntryRow::builder()
        .title(if secrets.get("youtube_client_id").is_some() { "Client ID (saved)" } else { "Client ID" }).build();
    let yt_secret = adw::PasswordEntryRow::builder()
        .title(if secrets.get("youtube_client_secret").is_some() { "Client secret (saved)" } else { "Client secret" }).build();
    let connect = gtk::Button::builder().label("Connect YouTube account").valign(gtk::Align::Center).build();
    let connect_row = adw::ActionRow::builder().title("Sign in")
        .subtitle(if s.data_path().map(|d| d.join("youtube-token.json").exists()).unwrap_or(false) { "Connected" } else { "Not connected" })
        .build();
    connect_row.add_suffix(&connect);
    yg.add(&yt_id);
    yg.add(&yt_secret);
    yg.add(&connect_row);
    if s.data_path().map(|d| d.join("youtube-token.json").exists()).unwrap_or(false) {
        let (row, s2) = (connect_row.clone(), s.clone());
        spawn(
            state,
            move || YouTube::from_settings(&s2.publish, &s2.data_path()?)?.channel(),
            move |_, res| {
                if let Ok((name, link)) = res {
                    row.set_subtitle(&format!("Connected to {name} · {link}"));
                }
            },
        );
    }
    pub_page.add(&yg);

    /* ---- Style ---- */
    let style_page = adw::PreferencesPage::builder().title("Style").icon_name("applications-graphics-symbolic").build();
    let r = &s.render;
    let st_rows = Rc::new(StyleRows {
        font_file: entry("Font file (.ttf/.otf, empty = system default)", &r.font_file),
        font_scale: spin("Text size (share of width)", 0.04, 0.14, 0.005, r.font_scale as f64, 3),
        position: combo("Text position", &POSITIONS, &r.text_position),
        uppercase: adw::SwitchRow::builder().title("Uppercase text").active(r.uppercase).build(),
        wrap: spin("Characters per line", 8.0, 30.0, 1.0, r.wrap_chars as f64, 0),
        text_color: entry("Text colour (name or 0xRRGGBB)", &r.text_color),
        text_box: adw::SwitchRow::builder().title("Box behind text").active(r.text_box).build(),
        box_color: entry("Box colour", &r.text_box_color),
        box_opacity: spin("Box opacity", 0.0, 1.0, 0.05, r.text_box_opacity as f64, 2),
        background: entry("Background colour", &r.background),
        background_image: entry("Background picture (path, optional)", &r.background_image),
        dim: spin("Darken picture", 0.0, 0.9, 0.05, r.background_dim as f64, 2),
        accent: entry("Accent colour (progress bar)", &r.accent),
        progress: adw::SwitchRow::builder().title("Progress bar").active(r.progress_bar).build(),
    });
    let tg = adw::PreferencesGroup::builder().title("Text").build();
    for w in [st_rows.font_file.upcast_ref::<gtk::Widget>(), st_rows.font_scale.upcast_ref(), st_rows.position.upcast_ref(),
              st_rows.uppercase.upcast_ref(), st_rows.wrap.upcast_ref(), st_rows.text_color.upcast_ref(),
              st_rows.text_box.upcast_ref(), st_rows.box_color.upcast_ref(), st_rows.box_opacity.upcast_ref()] {
        tg.add(w);
    }
    let bg = adw::PreferencesGroup::builder().title("Background").build();
    for w in [st_rows.background.upcast_ref::<gtk::Widget>(), st_rows.background_image.upcast_ref(), st_rows.dim.upcast_ref(),
              st_rows.accent.upcast_ref(), st_rows.progress.upcast_ref()] {
        bg.add(w);
    }
    let pv = adw::PreferencesGroup::builder().title("Preview").description("Rendered with the same engine as your videos.").build();
    let picture = gtk::Picture::builder().height_request(480).content_fit(gtk::ContentFit::Contain).build();
    let preview_btn = gtk::Button::builder().label("Update preview").halign(gtk::Align::Start).build();
    let pv_box = gtk::Box::builder().orientation(gtk::Orientation::Vertical).spacing(8).build();
    pv_box.append(&picture);
    pv_box.append(&preview_btn);
    pv.add(&pv_box);
    style_page.add(&pv);
    style_page.add(&tg);
    style_page.add(&bg);
    {
        let (st, rows, pic) = (state.clone(), st_rows.clone(), picture.clone());
        let render_preview = move |btn: &gtk::Button| {
            let base = st.settings.borrow().clone();
            let style = rows.read(&base.render);
            btn.set_sensitive(false);
            let (btn, pic) = (btn.clone(), pic.clone());
            spawn(
                &st,
                move || {
                    let dir = base.data_path()?.join("preview");
                    shorts_core::render::preview_frame("Your AI answers are bad because of THIS", &style, &dir, &dir.join("preview.png"))
                },
                move |st, res| {
                    btn.set_sensitive(true);
                    match res {
                        Ok(path) => pic.set_filename(Some(&path)),
                        Err(e) => st.toast(&format!("Preview failed: {e}")),
                    }
                },
            );
        };
        preview_btn.connect_clicked(render_preview);
    }

    /* ---- Music ---- */
    let music_page = adw::PreferencesPage::builder().title("Music").icon_name("audio-x-generic-symbolic").build();
    let mg = adw::PreferencesGroup::builder()
        .title("Royalty-free background music")
        .description("Use a folder of tracks you are allowed to use (YouTube Audio Library, Pixabay Music, Free Music Archive CC0/CC BY). Each file needs license info: a sidecar <file>.json with title, artist, license and url, or one library.json. Tracks without a license are never used. The credit goes into the description.")
        .build();
    let m_enabled = adw::SwitchRow::builder().title("Add background music").active(s.music.enabled).build();
    let m_folder = entry("Music folder", &s.music.folder);
    let m_volume = spin("Music level (dB)", -40.0, 0.0, 1.0, s.music.volume_db as f64, 0);
    let m_duck = adw::SwitchRow::builder().title("Lower music while the voice speaks").active(s.music.ducking).build();
    let count = if s.music.folder.is_empty() {
        "No folder set".to_string()
    } else {
        match shorts_core::music::scan(std::path::Path::new(&s.music.folder)) {
            Ok(t) => format!("{} licensed track(s) found", t.len()),
            Err(e) => e.to_string(),
        }
    };
    let m_count = adw::ActionRow::builder().title("Library").subtitle(count.as_str()).build();
    for w in [m_enabled.upcast_ref::<gtk::Widget>(), m_folder.upcast_ref(), m_volume.upcast_ref(), m_duck.upcast_ref(), m_count.upcast_ref()] {
        mg.add(w);
    }
    music_page.add(&mg);

    /* ---- Channel ---- */
    let ch = adw::PreferencesPage::builder().title("Channel").icon_name("video-display-symbolic").build();
    let cg = adw::PreferencesGroup::builder().title("Strategy").description("Every prompt uses this, so be specific.").build();
    let niche = entry("Niche", &s.strategy.niche);
    let audience = entry("Audience", &s.strategy.audience);
    let tone = entry("Tone", &s.strategy.tone);
    let language = entry("Language", &s.strategy.language);
    let cadence = entry("Posting cadence", &s.strategy.cadence);
    let orig = entry("What makes this channel original", &s.strategy.originality);
    for r in [&niche, &audience, &tone, &language, &cadence, &orig] {
        cg.add(r);
    }
    ch.add(&cg);

    for page in [&models, &routing, &voice_page, &style_page, &music_page, &pub_page, &ch] {
        dialog.add(page);
    }

    // Saves everything the dialog shows. Returns an error message for a toast.
    let save: Rc<dyn Fn() -> Result<(), String>> = {
        let st = state.clone();
        let (yt_id, yt_secret) = (yt_id.clone(), yt_secret.clone());
        Rc::new(move || {
            let mut secrets = Secrets::load();
            let mut s = st.settings.borrow().clone();
            for (p, r) in s.providers.iter_mut().zip(rows.iter()) {
                p.enabled = r.enabled.is_active();
                p.model = r.model.text().trim().to_string();
                p.daily_token_budget = r.budget.value().max(0.0) as u64;
                let url = r.base_url.text().trim().to_string();
                p.base_url = if url.is_empty() { None } else { Some(url) };
                if let Some(k) = &r.key {
                    let t = k.text();
                    if !t.trim().is_empty() {
                        secrets.set(&p.name, &t);
                    }
                }
            }
            let split = |e: &adw::EntryRow| e.text().split(',').map(|x| x.trim().to_string()).filter(|x| !x.is_empty()).collect::<Vec<_>>();
            s.roles.strategist = split(&strat);
            s.roles.writer = split(&writer);
            s.roles.critic = split(&critic);
            let param = vparam.text().trim().to_string();
            s.voice = match combo_value(&engine, &VOICES).as_str() {
                "piper" => VoiceEngine::Piper { model: param },
                "openai" => VoiceEngine::OpenAi { model: vmodel.text().trim().to_string(), voice: if param.is_empty() { "alloy".into() } else { param } },
                "none" => VoiceEngine::None,
                _ => VoiceEngine::Espeak { voice: if param.is_empty() { "en-us".into() } else { param } },
            };
            s.publish.target = combo_value(&target, &TARGETS);
            s.publish.auto_publish = auto.is_active();
            s.publish.min_score = min_score.value().clamp(1.0, 10.0) as u8;
            s.publish.youtube_privacy = combo_value(&privacy, &PRIVACY);
            for (name, row) in [("youtube_client_id", &yt_id), ("youtube_client_secret", &yt_secret)] {
                let t = row.text();
                if !t.trim().is_empty() {
                    secrets.set(name, &t);
                }
            }
            s.render = st_rows.read(&s.render);
            s.music.enabled = m_enabled.is_active();
            s.music.folder = m_folder.text().trim().to_string();
            s.music.volume_db = m_volume.value() as f32;
            s.music.ducking = m_duck.is_active();
            s.strategy.niche = niche.text().to_string();
            s.strategy.audience = audience.text().to_string();
            s.strategy.tone = tone.text().to_string();
            s.strategy.language = language.text().to_string();
            s.strategy.cadence = cadence.text().to_string();
            s.strategy.originality = orig.text().to_string();
            secrets.save().map_err(|e| format!("Could not save keys: {e}"))?;
            s.save().map_err(|e| format!("Could not save settings: {e}"))?;
            *st.settings.borrow_mut() = s;
            Ok(())
        })
    };

    {
        let (st, save, row) = (state.clone(), save.clone(), connect_row.clone());
        connect.connect_clicked(move |btn| {
            if let Err(e) = save() {
                st.toast(&e);
                return;
            }
            let s = st.settings.borrow().clone();
            btn.set_sensitive(false);
            row.set_subtitle("Waiting for Google sign-in in your browser…");
            let (btn, row) = (btn.clone(), row.clone());
            spawn(
                &st,
                move || {
                    let yt = YouTube::from_settings(&s.publish, &s.data_path()?)?;
                    yt.connect(|url| {
                        let _ = std::process::Command::new("xdg-open").arg(url).spawn();
                    })?;
                    yt.channel().map(|(name, _)| name)
                },
                move |st, res| {
                    btn.set_sensitive(true);
                    match res {
                        Ok(name) => { row.set_subtitle(&format!("Connected to {name}")); st.toast(&format!("YouTube connected: {name}")); }
                        Err(e) => { row.set_subtitle("Not connected"); st.toast(&e.to_string()); }
                    }
                },
            );
        });
    }

    {
        let st = state.clone();
        dialog.connect_closed(move |_| {
            match save() {
                Ok(()) => st.toast("Preferences saved"),
                Err(e) => st.toast(&e),
            }
            refresh(&st);
        });
    }

    dialog.present(Some(&state.window));
}
