//! Sidebar workspaces. Shorts is the board in `board.rs`; Assets is live;
//! the others say what they will do and which milestone brings them.

use crate::State;
use adw::prelude::*;
use gtk::gio;
use shorts_core::assets::{AssetKind, Library};
use std::rc::Rc;

/// (id, title, icon) in sidebar order.
pub const SIDEBAR: &[(&str, &str, &str)] = &[
    ("strategy", "Strategy", "x-office-calendar-symbolic"),
    ("shorts", "Shorts", "video-display-symbolic"),
    ("long", "Long video", "camera-video-symbolic"),
    ("decks", "Presentations", "x-office-presentation-symbolic"),
    ("design", "Design", "applications-graphics-symbolic"),
    ("assets", "Assets", "folder-music-symbolic"),
];

pub struct Planned {
    pub id: &'static str,
    pub title: &'static str,
    pub icon: &'static str,
    pub what: &'static str,
}

pub const PLANNED: &[Planned] = &[
    Planned {
        id: "strategy",
        title: "Strategy",
        icon: "x-office-calendar-symbolic",
        what: "Content pillars, a four-week calendar and a brief for every piece. One click turns a brief into a Short, long video, deck or design. A weekly review reads your YouTube stats. Arrives in milestone M5.",
    },
    Planned {
        id: "long",
        title: "Long video",
        icon: "camera-video-symbolic",
        what: "16:9 videos of 5–20 minutes: chapter outline, script per chapter, voiceover, licensed B-roll images, captions and SRT, and chapter timestamps in the description. Arrives in milestone M2.",
    },
    Planned {
        id: "decks",
        title: "Presentations",
        icon: "x-office-presentation-symbolic",
        what: "Slides from a brief or outline in your brand kit, exported as PDF and PNG, with an optional narrated video. Arrives in milestone M3.",
    },
    Planned {
        id: "design",
        title: "Design",
        icon: "applications-graphics-symbolic",
        what: "Thumbnails, social posts and channel banners in three variants each, scored for readability at small size, exported as PNG and PDF. Arrives in milestone M4.",
    },
];

pub fn planned_page(p: &Planned) -> adw::StatusPage {
    adw::StatusPage::builder().icon_name(p.icon).title(p.title).description(p.what).build()
}

/// Rebuilds the Assets page: licensed file counts per kind and a button to open each folder.
pub fn fill_assets(state: &Rc<State>, container: &gtk::Box) {
    while let Some(c) = container.first_child() {
        container.remove(&c);
    }
    let page = adw::PreferencesPage::new();
    let Ok(root) = state.settings.borrow().assets_path() else { return };
    let lib = Library::new(root);
    let _ = lib.ensure();
    let group = adw::PreferencesGroup::builder()
        .title("Asset library")
        .description("Put files you are allowed to use in these folders. Each file needs license info: a sidecar <file>.json with title, artist, license and url, or one library.json per folder. Files without it are never used. Credits are added to descriptions automatically.")
        .build();
    for (kind, title) in [(AssetKind::Music, "Music"), (AssetKind::Image, "Images"), (AssetKind::Font, "Fonts")] {
        let count = match lib.list(kind) {
            Ok(a) => format!("{} licensed file(s)", a.len()),
            Err(e) => e.to_string(),
        };
        let row = adw::ActionRow::builder().title(title).subtitle(count.as_str()).build();
        let open = gtk::Button::builder().label("Open folder").valign(gtk::Align::Center).build();
        let folder = lib.folder(kind);
        let window = state.window.clone();
        open.connect_clicked(move |_| {
            let launcher = gtk::FileLauncher::new(Some(&gio::File::for_path(&folder)));
            launcher.launch(Some(&window), gio::Cancellable::NONE, |_| {});
        });
        row.add_suffix(&open);
        group.add(&row);
    }
    page.add(&group);
    page.set_vexpand(true);
    container.append(&page);
}
