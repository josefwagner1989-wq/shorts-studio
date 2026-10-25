//! Creator Studio — GTK4 / libadwaita front end over `shorts-core`.
//! Long work (model calls, voice, ffmpeg, uploads) runs on worker threads via
//! `gio::spawn_blocking`; the UI only reads and writes the store.

mod board;
mod prefs;
mod workspaces;

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
    // Create the asset library folders (music, images, fonts) so they are easy to find.
    if let Ok(p) = settings.assets_path() {
        let _ = shorts_core::assets::Library::new(p).ensure();
    }

    // Shorts workspace: header with its actions, and the pipeline board.
    let header = adw::HeaderBar::new();
    let plan_btn = gtk::Button::builder().label("Plan ideas").css_classes(["suggested-action"]).build();
    let run_btn = gtk::Button::builder().label("Produce all").tooltip_text("Script, voice and render every unfinished Short").build();
    header.pack_start(&plan_btn);
    header.pack_start(&run_btn);

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

    let stack = gtk::Stack::builder().transition_type(gtk::StackTransitionType::Crossfade).build();
    stack.add_named(&scroller, Some("shorts"));
    for ws in workspaces::PLANNED {
        stack.add_named(&workspaces::planned_page(ws), Some(ws.id));
    }
    let assets_box = gtk::Box::builder().orientation(gtk::Orientation::Vertical).build();
    stack.add_named(&assets_box, Some("assets"));

    let content_view = adw::ToolbarView::new();
    content_view.add_top_bar(&header);
    content_view.set_content(Some(&stack));
    let content_page = adw::NavigationPage::new(&content_view, "Shorts");

    // Sidebar
    let side_header = adw::HeaderBar::new();
    let prefs_btn = gtk::Button::builder().icon_name("emblem-system-symbolic").tooltip_text("Preferences").build();
    side_header.pack_end(&prefs_btn);
    let nav = gtk::ListBox::builder().css_classes(["navigation-sidebar"]).build();
    for (id, title, icon) in workspaces::SIDEBAR {
        let row = gtk::Box::builder().spacing(12).margin_top(6).margin_bottom(6).margin_start(6).build();
        row.append(&gtk::Image::from_icon_name(icon));
        row.append(&gtk::Label::new(Some(*title)));
        let lbr = gtk::ListBoxRow::builder().child(&row).name(*id).build();
        nav.append(&lbr);
    }
    let side_view = adw::ToolbarView::new();
    side_view.add_top_bar(&side_header);
    side_view.set_content(Some(&gtk::ScrolledWindow::builder().child(&nav).vexpand(true).build()));
    let side_page = adw::NavigationPage::new(&side_view, "Creator Studio");

    let split = adw::NavigationSplitView::new();
    split.set_sidebar(Some(&side_page));
    split.set_content(Some(&content_page));
    split.set_min_sidebar_width(200.0);

    let toasts = adw::ToastOverlay::new();
    toasts.set_child(Some(&split));

    let window = adw::ApplicationWindow::builder()
        .application(app)
        .title("Creator Studio")
        .default_width(1360)
        .default_height(800)
        .content(&toasts)
        .build();
    let bp = adw::Breakpoint::new(adw::BreakpointCondition::new_length(
        adw::BreakpointConditionLengthType::MaxWidth, 720.0, adw::LengthUnit::Sp));
    bp.add_setter(&split, "collapsed", Some(&true.to_value()));
    window.add_breakpoint(bp);

    let state = Rc::new(State {
        settings: RefCell::new(settings),
        window: window.clone(),
        toasts,
        board,
        busy: RefCell::new(false),
    });

    board::refresh(&state);

    {
        let (st, stack, page, plan_btn, run_btn, split, assets_box) =
            (state.clone(), stack.clone(), content_page.clone(), plan_btn.clone(), run_btn.clone(), split.clone(), assets_box.clone());
        nav.connect_row_selected(move |_, row| {
            let Some(row) = row else { return };
            let id = row.widget_name().to_string();
            let title = workspaces::SIDEBAR.iter().find(|w| w.0 == id).map(|w| w.1).unwrap_or("Creator Studio");
            page.set_title(title);
            let is_shorts = id == "shorts";
            plan_btn.set_visible(is_shorts);
            run_btn.set_visible(is_shorts);
            if id == "assets" {
                workspaces::fill_assets(&st, &assets_box);
            }
            stack.set_visible_child_name(&id);
            split.set_show_content(true);
        });
    }
    if let Some(first) = nav.row_at_index(1) {
        nav.select_row(Some(&first));
    }

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
