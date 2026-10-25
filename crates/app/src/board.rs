//! The pipeline board (one column per stage) and the per-video dialog.

use crate::State;
use adw::prelude::*;
use gtk::{gio, glib};
use shorts_core::config::Settings;
use shorts_core::model::{Stage, Video};
use shorts_core::router::Router;
use shorts_core::store::Store;
use shorts_core::{agents, publish, workflow, Error, Result};
use std::rc::Rc;

/// Runs `work` on a worker thread, then `done` on the UI thread.
pub fn spawn<T: Send + 'static>(
    state: &Rc<State>,
    work: impl FnOnce() -> Result<T> + Send + 'static,
    done: impl FnOnce(&Rc<State>, Result<T>) + 'static,
) {
    let st = state.clone();
    glib::spawn_future_local(async move {
        let res = gio::spawn_blocking(work)
            .await
            .unwrap_or_else(|_| Err(Error::Config("the background task crashed".into())));
        done(&st, res);
    });
}

fn store_for(s: &Settings) -> Result<Store> {
    Store::open(&s.data_path()?)
}

fn label(text: &str, classes: &[&str]) -> gtk::Label {
    let l = gtk::Label::builder().label(text).wrap(true).xalign(0.0).selectable(true).build();
    for c in classes {
        l.add_css_class(c);
    }
    l
}

pub fn refresh(state: &Rc<State>) {
    while let Some(child) = state.board.first_child() {
        state.board.remove(&child);
    }
    let videos = store_for(&state.settings.borrow()).and_then(|s| s.list()).unwrap_or_default();

    for stage in Stage::BOARD.iter().copied().chain(std::iter::once(Stage::Failed)) {
        let items: Vec<&Video> = videos.iter().filter(|v| v.stage == stage).collect();
        if stage == Stage::Failed && items.is_empty() {
            continue;
        }
        let col = gtk::Box::builder().orientation(gtk::Orientation::Vertical).spacing(8).width_request(220).build();
        col.append(&label(&format!("{}  ·  {}", stage.label(), items.len()), &["heading"]));

        let list = gtk::ListBox::builder().selection_mode(gtk::SelectionMode::None).css_classes(["boxed-list"]).build();
        if items.is_empty() {
            list.append(&label(if stage == Stage::Idea { "No ideas yet. Press Plan ideas." } else { "Empty" }, &["dim-label"]));
        }
        for v in items {
            let title = v.idea.as_ref().map(|i| i.title.clone()).unwrap_or_else(|| v.id.clone());
            let mut sub = v.idea.as_ref().map(|i| i.hook.clone()).unwrap_or_default();
            if let Some(c) = &v.critique {
                sub = format!("critic {}/10 · {}", c.score, sub);
            }
            let row = adw::ActionRow::builder().title(glib::markup_escape_text(&title).as_str())
                .subtitle(glib::markup_escape_text(&sub).as_str()).subtitle_lines(2).activatable(true).build();
            let st = state.clone();
            let id = v.id.clone();
            row.connect_activated(move |_| open_detail(&st, &id));
            list.append(&row);
        }
        let scroll = gtk::ScrolledWindow::builder().vexpand(true).hscrollbar_policy(gtk::PolicyType::Never).child(&list).build();
        col.append(&scroll);
        state.board.append(&col);
    }
}

pub fn plan(state: &Rc<State>, btn: gtk::Button) {
    btn.set_sensitive(false);
    state.toast("Planning ideas…");
    let s = state.settings.borrow().clone();
    spawn(
        state,
        move || {
            let store = store_for(&s)?;
            let router = Router::from_settings(&s)?;
            let existing: Vec<String> = store.list()?.iter().filter_map(|v| v.idea.as_ref().map(|i| i.title.clone())).collect();
            let ideas = agents::plan_ideas(&router, &s.strategy, &existing, 5)?;
            let n = ideas.len();
            for i in ideas {
                workflow::new_video(&store, i)?;
            }
            Ok(n)
        },
        move |st, res| {
            btn.set_sensitive(true);
            match res {
                Ok(n) => st.toast(&format!("Added {n} ideas")),
                Err(e) => st.toast(&format!("Planning stopped: {e}")),
            }
            refresh(st);
        },
    );
}

fn run_ids(state: &Rc<State>, ids: Vec<String>, publish_now: bool, done_msg: &'static str, on_done: impl FnOnce() + 'static) {
    if *state.busy.borrow() {
        state.toast("Already working on a video. Wait for it to finish.");
        on_done();
        return;
    }
    *state.busy.borrow_mut() = true;
    let s = state.settings.borrow().clone();
    spawn(
        state,
        move || {
            let store = store_for(&s)?;
            let router = Router::from_settings(&s)?;
            let publisher = if publish_now || s.publish.auto_publish { Some(publish::from_settings(&s)?) } else { None };
            let mut errors = Vec::new();
            for id in ids {
                let v = store.load(&id)?;
                let opts = workflow::RunOptions { allow_publish: publish_now || s.publish.auto_publish, ..Default::default() };
                if let Err(e) = workflow::run_video(&s, &router, &store, publisher.as_deref(), v, opts, &|_| {}) {
                    errors.push(e.to_string());
                }
            }
            Ok(errors)
        },
        move |st, res| {
            *st.busy.borrow_mut() = false;
            on_done();
            match res {
                Ok(errs) if errs.is_empty() => st.toast(done_msg),
                Ok(errs) => st.toast(&format!("{} video(s) stopped: {}", errs.len(), errs[0])),
                Err(e) => st.toast(&format!("Stopped: {e}")),
            }
            refresh(st);
        },
    );
}

pub fn produce_all(state: &Rc<State>, btn: gtk::Button) {
    let ids: Vec<String> = store_for(&state.settings.borrow())
        .and_then(|s| s.list())
        .unwrap_or_default()
        .into_iter()
        .filter(|v| matches!(v.stage, Stage::Idea | Stage::Script | Stage::Voiced))
        .map(|v| v.id)
        .collect();
    if ids.is_empty() {
        state.toast("Nothing to produce. Plan some ideas first.");
        return;
    }
    btn.set_sensitive(false);
    state.toast(&format!("Producing {} video(s)…", ids.len()));
    run_ids(state, ids, false, "All videos produced", move || btn.set_sensitive(true));
}

fn open_detail(state: &Rc<State>, id: &str) {
    let Ok(store) = store_for(&state.settings.borrow()) else { return };
    let Ok(v) = store.load(id) else { return };
    let title = v.idea.as_ref().map(|i| i.title.clone()).unwrap_or_else(|| v.id.clone());

    let body = gtk::Box::builder().orientation(gtk::Orientation::Vertical).spacing(10)
        .margin_top(12).margin_bottom(18).margin_start(18).margin_end(18).build();
    body.append(&label(&format!("Stage: {}", v.stage.label()), &["dim-label"]));
    if let Some(i) = &v.idea {
        body.append(&label("Hook", &["heading"]));
        body.append(&label(&i.hook, &[]));
        body.append(&label(&i.angle, &["dim-label"]));
    }
    if let Some(c) = &v.critique {
        body.append(&label(&format!("Critic {}/10 · originality risk {}", c.score, c.originality_risk), &["heading"]));
        body.append(&label(&c.summary, &[]));
        for f in &c.fixes {
            body.append(&label(&format!("• {f}"), &["dim-label"]));
        }
    }
    if let Some(sc) = &v.script {
        body.append(&label(&format!("Script · {:.0} s", sc.total_seconds()), &["heading"]));
        let mut t = 0.0;
        for s in &sc.scenes {
            body.append(&label(&format!("{:>4.1}s  {}\n        🎙 {}", t, s.on_screen, s.voiceover), &["monospace"]));
            t += s.seconds;
        }
        body.append(&label(&format!("{}\n{}", sc.caption, sc.hashtags.join(" ")), &["dim-label"]));
    }
    if let Some(u) = &v.published_url {
        body.append(&label(&format!("Published: {u}"), &[]));
    }
    if !v.log.is_empty() {
        body.append(&label("Activity", &["heading"]));
        let start = v.log.len().saturating_sub(8);
        body.append(&label(&v.log[start..].join("\n"), &["dim-label", "caption"]));
    }

    let actions = gtk::Box::builder().orientation(gtk::Orientation::Horizontal).spacing(8).build();
    body.append(&actions);

    let dialog = adw::Dialog::builder().title(title.as_str()).content_width(620).content_height(700).build();

    let add = |text: &str, suggested: bool| {
        let b = gtk::Button::with_label(text);
        if suggested {
            b.add_css_class("suggested-action");
        }
        actions.append(&b);
        b
    };

    if matches!(v.stage, Stage::Idea | Stage::Script | Stage::Voiced | Stage::Failed) {
        let b = add("Produce", true);
        let (st, d, id) = (state.clone(), dialog.clone(), v.id.clone());
        b.connect_clicked(move |b| {
            b.set_sensitive(false);
            st.toast("Producing… this takes a minute or two.");
            d.close();
            run_ids(&st, vec![id.clone()], false, "Video produced", || {});
        });
    }
    if v.stage == Stage::Rendered {
        let b = add("Approve", true);
        let (st, d, id) = (state.clone(), dialog.clone(), v.id.clone());
        b.connect_clicked(move |_| {
            let res = store_for(&st.settings.borrow()).and_then(|s| { let v = s.load(&id)?; workflow::approve(&s, v) });
            match res {
                Ok(_) => st.toast("Approved"),
                Err(e) => st.toast(&e.to_string()),
            }
            d.close();
            refresh(&st);
        });
    }
    if v.stage == Stage::Approved {
        let target = state.settings.borrow().publish.target.clone();
        let b = add(&format!("Publish to {target}"), true);
        let (st, d, id) = (state.clone(), dialog.clone(), v.id.clone());
        b.connect_clicked(move |_| {
            d.close();
            st.toast("Publishing…");
            run_ids(&st, vec![id.clone()], true, "Published", || {});
        });
    }
    if let Some(file) = v.video_file.clone() {
        let b = add("Play", false);
        let st = state.clone();
        b.connect_clicked(move |_| {
            let launcher = gtk::FileLauncher::new(Some(&gio::File::for_path(&file)));
            launcher.launch(Some(&st.window), gio::Cancellable::NONE, |_| {});
        });
    }
    {
        let b = add("Delete", false);
        b.add_css_class("destructive-action");
        let (st, d, id) = (state.clone(), dialog.clone(), v.id.clone());
        let armed = std::cell::Cell::new(false);
        b.connect_clicked(move |b| {
            if !armed.get() {
                armed.set(true);
                b.set_label("Click again to delete");
                return;
            }
            if let Ok(s) = store_for(&st.settings.borrow()) {
                let _ = s.delete(&id);
            }
            d.close();
            refresh(&st);
        });
    }

    let toolbar = adw::ToolbarView::new();
    toolbar.add_top_bar(&adw::HeaderBar::new());
    toolbar.set_content(Some(&gtk::ScrolledWindow::builder().child(&body).vexpand(true).build()));
    dialog.set_child(Some(&toolbar));
    dialog.present(Some(&state.window));
}
