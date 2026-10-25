//! Shorts Studio — GTK4 / libadwaita front end over `shorts-core`.
//! Long work (model calls, voice, ffmpeg, uploads) runs on worker threads via
//! `gio::spawn_blocking`; the UI only reads and writes the store.

mod board;
mod prefs;

use adw::prelude::*;
use gtk::glib;
use shorts_core::config::Settings;
use std::cell::RefCell;
use std::rc::Rc;

pub const APP_ID: &str = "io.github.shortsstudio.ShortsStudio";

/// Shared UI state.
pub struct State {
    pub settings: RefCell<Settings>,
    pub window: adw::ApplicationWindow,
    pub toasts: adw::ToastOverlay,
    pub board: gtk::Box,
    pub busy: RefCell<bool>,
}

impl State {
    pub fn toast(&self, text: &str) {
        self.toasts.add_toast(adw::Toast::new(text));
    }
}

fn main() -> glib::ExitCode {
    let app = adw::Application::builder().application_id(APP_ID).build();
    app.connect_activate(build_ui);
    app.run()
}

fn build_ui(app: &adw::Application) {
    let settings = Settings::load_or_default().unwrap_or_default();

    let header = adw::HeaderBar::new();
    let plan_btn = gtk::Button::builder().label("Plan ideas").css_classes(["suggested-action"]).build();
    let run_btn = gtk::Button::builder().label("Produce all").tooltip_text("Script, voice and render every unfinished video").build();
    let prefs_btn = gtk::Button::builder().icon_name("emblem-system-symbolic").tooltip_text("Preferences").build();
    header.pack_start(&plan_btn);
    header.pack_start(&run_btn);
    header.pack_end(&prefs_btn);

    let board = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(12)
        .margin_top(12).margin_bottom(12).margin_start(12).margin_end(12)
        .homogeneous(true)
        .build();
    let scroller = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Automatic)
        .vscrollbar_policy(gtk::PolicyType::Never)
        .child(&board)
        .vexpand(true)
        .build();

    let toasts = adw::ToastOverlay::new();
    toasts.set_child(Some(&scroller));

    let view = adw::ToolbarView::new();
    view.add_top_bar(&header);
    view.set_content(Some(&toasts));

    let window = adw::ApplicationWindow::builder()
        .application(app)
        .title("Shorts Studio")
        .default_width(1280)
        .default_height(780)
        .content(&view)
        .build();

    let state = Rc::new(State {
        settings: RefCell::new(settings),
        window: window.clone(),
        toasts,
        board,
        busy: RefCell::new(false),
    });

    board::refresh(&state);

    {
        let st = state.clone();
        prefs_btn.connect_clicked(move |_| prefs::open(&st));
    }
    {
        let st = state.clone();
        plan_btn.connect_clicked(move |btn| board::plan(&st, btn.clone()));
    }
    {
        let st = state.clone();
        run_btn.connect_clicked(move |btn| board::produce_all(&st, btn.clone()));
    }

    // First run without any key: open Preferences so the user can add one.
    let has_key = {
        let s = state.settings.borrow();
        let secrets = shorts_core::secrets::Secrets::load();
        s.providers.iter().any(|p| {
            p.enabled && (p.kind == shorts_core::config::ProviderKind::Ollama
                || secrets.resolve(&p.name, p.api_key_env.as_deref()).is_some())
        })
    };
    window.present();
    if !has_key {
        state.toast("Add at least one API key, or enable a local Ollama model, to start.");
        prefs::open(&state);
    }
}
