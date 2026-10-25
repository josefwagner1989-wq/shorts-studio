//! Preferences: API keys and usage limits per provider, role routing, voice,
//! publishing and channel strategy. Saved when the dialog closes.

use crate::board::{refresh, spawn};
use crate::State;
use adw::prelude::*;
use gtk::glib;
use shorts_core::config::{ProviderKind, VoiceEngine};
use shorts_core::publish::YouTube;
use shorts_core::secrets::Secrets;
use std::rc::Rc;

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
    pub_page.add(&yg);

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

    for page in [&models, &routing, &voice_page, &pub_page, &ch] {
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
                    })
                },
                move |st, res| {
                    btn.set_sensitive(true);
                    match res {
                        Ok(()) => { row.set_subtitle("Connected"); st.toast("YouTube connected"); }
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
