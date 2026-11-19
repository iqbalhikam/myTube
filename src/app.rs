use crate::model::*;
use crate::{brave, catalog, sponsor, store, DATA_DIR};
use egui::{pos2, vec2, Align, Align2, Color32, FontId, Frame, Layout, Margin, Rect, RichText, Rounding, Sense, Stroke};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::time::{Duration, Instant};

const RED: Color32 = Color32::from_rgb(255, 45, 77);
const MAGENTA: Color32 = Color32::from_rgb(255, 61, 174);
const VIOLET: Color32 = Color32::from_rgb(124, 92, 255);
const BLUE: Color32 = Color32::from_rgb(46, 155, 255);
const GREEN: Color32 = Color32::from_rgb(34, 229, 132);
const CYAN: Color32 = Color32::from_rgb(25, 211, 218);
const AMBER: Color32 = Color32::from_rgb(255, 176, 32);
const WHITE: Color32 = Color32::from_rgb(255, 255, 255);

#[derive(Clone, Copy)]
struct Pal {
    bg: Color32,
    surface: Color32,
    surface2: Color32,
    ink: Color32,
    muted: Color32,
    line: Color32,
}

#[derive(Clone)]
struct Snack {
    msg: String,
    undo: Option<UndoAction>,
    until: Instant,
}

pub struct NoIklanApp {
    state: State,
    catalog: Vec<Video>,
    tab: Tab,
    lib_tab: LibraryTab,
    search: String,
    category: String,
    sort: String,
    lib_sort: String,
    lib_search: String,
    detail: Option<String>,
    playlist_detail: Option<String>,
    dialog: Option<Dialog>,
    snack: Option<Snack>,
    data_path: PathBuf,
    dirty: bool,
    last_save: Instant,
    tx: Sender<AsyncResult>,
    rx: Receiver<AsyncResult>,
    pending: HashSet<String>,
    segments: HashMap<String, Vec<Segment>>,
    new_playlist_name: String,
    rename_name: String,
    link_input: String,
    selected: HashSet<String>,
}

impl NoIklanApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        let data_path = DATA_DIR.get().cloned().unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
        let state = store::load(&data_path);
        let (tx, rx) = channel();
        Self {
            state,
            catalog: catalog::videos(),
            tab: Tab::Home,
            lib_tab: LibraryTab::History,
            search: String::new(),
            category: "Semua".into(),
            sort: "Relevansi".into(),
            lib_sort: "Urutan".into(),
            lib_search: String::new(),
            detail: None,
            playlist_detail: None,
            dialog: None,
            snack: None,
            data_path,
            dirty: false,
            last_save: Instant::now(),
            tx,
            rx,
            pending: HashSet::new(),
            segments: HashMap::new(),
            new_playlist_name: String::new(),
            rename_name: String::new(),
            link_input: String::new(),
            selected: HashSet::new(),
        }
    }

    fn pal(&self) -> Pal {
        if self.state.settings.theme == "light" {
            Pal { bg: Color32::from_rgb(246, 242, 235), surface: Color32::from_rgb(255, 255, 255), surface2: Color32::from_rgb(236, 230, 220), ink: Color32::from_rgb(25, 28, 36), muted: Color32::from_rgb(106, 112, 128), line: Color32::from_rgb(210, 202, 190) }
        } else {
            Pal { bg: Color32::from_rgb(11, 14, 20), surface: Color32::from_rgb(17, 21, 31), surface2: Color32::from_rgb(26, 32, 48), ink: Color32::from_rgb(244, 241, 234), muted: Color32::from_rgb(139, 147, 167), line: Color32::from_rgb(50, 58, 78) }
        }
    }

    fn videos(&self) -> Vec<Video> {
        let mut v = self.catalog.clone();
        v.extend(self.state.custom_videos.iter().cloned());
        v
    }

    fn video(&self, id: &str) -> Option<Video> {
        self.catalog.iter().find(|v| v.id == id).cloned().or_else(|| self.state.custom_videos.iter().find(|v| v.id == id).cloned())
    }

    fn mark(&mut self) {
        self.dirty = true;
    }

    fn toast(&mut self, msg: impl Into<String>) {
        self.snack = Some(Snack { msg: msg.into(), undo: None, until: Instant::now() + Duration::from_secs(4) });
    }

    fn toast_undo(&mut self, msg: impl Into<String>, undo: UndoAction) {
        self.snack = Some(Snack { msg: msg.into(), undo: Some(undo), until: Instant::now() + Duration::from_secs(6) });
    }

    fn channels(&self) -> Vec<(String, String, usize)> {
        let mut m: HashMap<String, (String, usize)> = HashMap::new();
        for v in self.videos() {
            let e = m.entry(v.channel_id.clone()).or_insert((v.channel.clone(), 0));
            e.1 += 1;
        }
        let mut out: Vec<(String, String, usize)> = m.into_iter().map(|(id, (name, n))| (id, name, n)).collect();
        out.sort_by(|a, b| a.1.cmp(&b.1));
        out
    }

    fn playlist_mut(&mut self, pid: &str) -> Option<&mut Playlist> {
        self.state.playlists.iter_mut().find(|p| p.id == pid)
    }

    fn create_playlist(&mut self, name: &str) -> String {
        let id = format!("pl{}-{}", now(), self.state.playlists.len());
        self.state.playlists.push(Playlist { id: id.clone(), name: name.to_string(), ids: Vec::new(), created: now() });
        self.mark();
        self.toast(format!("Playlist dibuat: {}", name));
        id
    }

    fn open_detail(&mut self, id: &str) {
        self.detail = Some(id.to_string());
        if self.state.settings.sponsor && !self.segments.contains_key(id) {
            let key = format!("s:{}", id);
            if !self.pending.contains(&key) {
                self.pending.insert(key);
                let tx = self.tx.clone();
                let id2 = id.to_string();
                std::thread::spawn(move || {
                    let r = sponsor::fetch_segments(&id2);
                    let _ = tx.send(AsyncResult::Sponsor { id: id2, result: r });
                });
            }
        }
    }

    fn play(&mut self, ui: &egui::Ui, id: &str) {
        let v = match self.video(id) { Some(v) => v, None => return };
        let url = format!("https://www.youtube.com/watch?v={}", v.id);
        if !self.state.settings.background {
            ui.ctx().output_mut(|o| o.copied_text = url.clone());
            self.toast("Link disalin. Tempel di browser untuk memutar.");
        } else {
            let pref = if self.state.settings.block_ads { self.state.settings.brave_package.clone() } else { "default".to_string() };
            match brave::open_url(&url, &pref) {
                Ok(pkg) => {
                    let est = if self.state.settings.block_ads && pkg.contains("brave") { 3 } else { 0 };
                    if self.state.stats.last_day != day() {
                        self.state.stats.today = 0;
                        self.state.stats.last_day = day();
                    }
                    self.state.stats.opened += 1;
                    self.state.stats.blocked += est;
                    self.state.stats.today += est;
                    self.state.stats.time_saved_min += est / 3;
                    self.state.stats.data_saved_mb += est * 4;
                    if est > 0 {
                        *self.state.stats.per_category.entry(v.category.clone()).or_insert(0) += est;
                    }
                    if est > 0 {
                        self.toast(format!("Dibuka di Brave. {} iklan diblokir.", est));
                    } else if pkg == "default" {
                        self.toast("Dibuka di browser default (tanpa blokir iklan).");
                    } else {
                        self.toast("Dibuka di browser.");
                    }
                }
                Err(e) => {
                    self.toast(format!("Gagal membuka browser: {}", e));
                }
            }
        }
        self.state.history.retain(|h| h.id != v.id);
        self.state.history.insert(0, HistoryItem { id: v.id.clone(), at: now(), progress: 0.0 });
        self.state.history.truncate(100);
        self.mark();
    }

    fn toggle_like(&mut self, id: &str) {
        if self.state.likes.get(id).copied().unwrap_or(false) {
            self.state.likes.remove(id);
            self.mark();
            self.toast_undo("Dihapus dari Disukai", UndoAction::RestoreLike(id.to_string()));
        } else {
            self.state.likes.insert(id.to_string(), true);
            self.mark();
            self.toast("Ditambahkan ke Disukai");
        }
    }

    fn toggle_watch_later(&mut self, id: &str) {
        if self.state.watch_later.contains(&id.to_string()) {
            self.state.watch_later.retain(|x| x != id);
            self.mark();
            self.toast_undo("Dihapus dari Tonton nanti", UndoAction::RestoreWatchLater(id.to_string()));
        } else {
            self.state.watch_later.push(id.to_string());
            self.mark();
            self.toast("Disimpan ke Tonton nanti");
        }
    }

    fn toggle_sub(&mut self, cid: &str) {
        let on = {
            let e = self.state.subs.entry(cid.to_string()).or_insert(false);
            *e = !*e;
            *e
        };
        self.mark();
        self.toast(if on { "Berlangganan channel" } else { "Berhenti berlangganan" });
    }

    fn add_recent(&mut self, q: &str) {
        let q = q.trim().to_string();
        if q.is_empty() { return; }
        self.state.recents.retain(|x| x != &q);
        self.state.recents.insert(0, q);
        self.state.recents.truncate(8);
        self.mark();
    }

    fn search_results(&self) -> Vec<Video> {
        let q = self.search.trim().to_lowercase();
        let mut out: Vec<Video> = self.videos().into_iter().filter(|v| q.is_empty() || v.title.to_lowercase().contains(&q) || v.channel.to_lowercase().contains(&q) || v.category.to_lowercase().contains(&q)).collect();
        match self.sort.as_str() {
            "Terpopuler" => out.sort_by(|a, b| b.views.cmp(&a.views)),
            "Durasi" => out.sort_by(|a, b| b.duration.cmp(&a.duration)),
            "Terbaru" => out.sort_by(|a, b| b.custom.cmp(&a.custom)),
            _ => {}
        }
        out
    }

    fn apply_undo(&mut self, u: UndoAction) {
        match u {
            UndoAction::RestoreHistory(h) => self.state.history = h,
            UndoAction::RestorePlaylist(pl) => {
                if let Some(p) = self.playlist_mut(&pl.id) { *p = pl; } else { self.state.playlists.push(pl); }
            }
            UndoAction::RestoreLike(id) => { self.state.likes.insert(id, true); }
            UndoAction::RestoreWatchLater(id) => {
                if !self.state.watch_later.contains(&id) { self.state.watch_later.push(id); }
            }
            UndoAction::RestoreState(s) => self.state = *s,
        }
        self.mark();
    }

    fn poll_async(&mut self, ctx: &egui::Context) {
        let mut any = false;
        while let Ok(res) = self.rx.try_recv() {
            any = true;
            match res {
                AsyncResult::OEmbed { id, result } => {
                    self.pending.remove(&format!("o:{}", id));
                    match result {
                        Ok(v) => {
                            if self.video(&id).is_none() {
                                self.state.custom_videos.push(v);
                                self.mark();
                            }
                            self.open_detail(&id);
                            self.toast("Video ditambahkan ke katalog");
                        }
                        Err(e) => self.toast(format!("Gagal mengambil info video: {}", e)),
                    }
                }
                AsyncResult::Sponsor { id, result } => {
                    self.pending.remove(&format!("s:{}", id));
                    self.segments.insert(id, result.unwrap_or_default());
                }
            }
        }
        if any { ctx.request_repaint(); }
    }

    fn autosave(&mut self) {
        if self.dirty && self.last_save.elapsed() > Duration::from_millis(800) {
            let _ = store::save(&self.data_path, &self.state);
            self.dirty = false;
            self.last_save = Instant::now();
        }
    }

    fn apply_style(&self, ctx: &egui::Context) {
        let pal = self.pal();
        let mut v = if self.state.settings.theme == "light" { egui::Visuals::light() } else { egui::Visuals::dark() };
        v.panel_fill = pal.bg;
        v.window_fill = pal.surface;
        v.extreme_bg_color = pal.surface2;
        v.selection.bg_fill = RED;
        v.selection.stroke = Stroke::new(1.0, WHITE);
        v.hyperlink_color = BLUE;
        v.window_rounding = Rounding::same(16.0);
        v.widgets.noninteractive.bg_fill = pal.surface;
        v.widgets.inactive.bg_fill = pal.surface2;
        v.widgets.hovered.bg_fill = pal.surface2;
        v.widgets.active.bg_fill = RED;
        ctx.set_visuals(v);
        ctx.style_mut(|s| {
            s.spacing.item_spacing = vec2(8.0, 8.0);
            s.spacing.button_padding = vec2(12.0, 8.0);
        });
    }

    fn top_bar(&mut self, ui: &mut egui::Ui, pal: &Pal) {
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.label(RichText::new("NoIklan").size(22.0).strong().color(pal.ink));
                ui.label(RichText::new("YOUTUBE TANPA IKLAN").size(9.0).strong().color(GREEN));
            });
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui.button("Setelan").clicked() { self.dialog = Some(Dialog::Settings); }
                let shield = format!("{} diblokir", self.state.stats.blocked);
                if ui.add(egui::Button::new(RichText::new(shield).size(12.0).strong().color(GREEN)).fill(pal.surface2)).clicked() {
                    self.dialog = Some(Dialog::Stats);
                }
            });
        });
    }

    fn tab_bar(&mut self, ui: &mut egui::Ui, pal: &Pal) {
        ui.horizontal(|ui| {
            let tabs = [(Tab::Home, "Beranda"), (Tab::Search, "Cari"), (Tab::Subs, "Langganan"), (Tab::Library, "Pustaka")];
            let w = (ui.available_width() / 4.0).max(60.0);
            for (t, label) in tabs {
                let on = self.tab == t && self.detail.is_none() && self.playlist_detail.is_none();
                let color = if on { RED } else { pal.muted };
                let fill = if on { pal.surface2 } else { pal.surface };
                if ui.add(egui::Button::new(RichText::new(label).size(13.0).strong().color(color)).fill(fill).min_size(vec2(w - 6.0, 40.0))).clicked() {
                    self.tab = t;
                    self.detail = None;
                    self.playlist_detail = None;
                    self.selected.clear();
                }
            }
        });
    }

    fn home_screen(&mut self, ui: &mut egui::Ui, pal: &Pal) {
        let mut cats: Vec<String> = vec!["Semua".into()];
        for v in self.videos() {
            if !cats.contains(&v.category) { cats.push(v.category.clone()); }
        }
        egui::ScrollArea::horizontal().show(ui, |ui| {
            ui.horizontal(|ui| {
                for cat in cats {
                    let on = self.category == cat;
                    if ui.add(egui::Button::new(RichText::new(&cat).size(12.0).color(if on { WHITE } else { pal.ink })).fill(if on { RED } else { pal.surface2 })).clicked() {
                        self.category = cat.clone();
                    }
                }
            });
        });
        ui.add_space(6.0);
        if let Some(h) = self.state.history.first().cloned() {
            if h.progress < 0.95 {
                if let Some(v) = self.video(&h.id) {
                    Frame::none().fill(pal.surface).rounding(14.0).inner_margin(Margin::same(10.0)).show(ui, |ui| {
                        ui.horizontal(|ui| {
                            let (rect, resp) = ui.allocate_exact_size(vec2(96.0, 54.0), Sense::click());
                            paint_thumb(ui, rect, &v, true);
                            if resp.clicked() { self.open_detail(&v.id); }
                            ui.vertical(|ui| {
                                ui.label(RichText::new("LANJUTKAN MENONTON").size(9.0).strong().color(RED));
                                if ui.link(RichText::new(trunc(&v.title, 40)).size(13.0).strong().color(pal.ink)).clicked() { self.open_detail(&v.id); }
                                ui.label(RichText::new(format!("{} • {}", v.channel, fmt_dur(v.duration))).size(11.0).color(pal.muted));
                            });
                        });
                    });
                    ui.add_space(8.0);
                }
            }
        }
        ui.label(RichText::new("Trending").size(15.0).strong().color(pal.ink));
        let mut trend = self.videos();
        trend.sort_by(|a, b| b.views.cmp(&a.views));
        trend.truncate(6);
        egui::ScrollArea::horizontal().show(ui, |ui| {
            ui.horizontal(|ui| {
                for v in trend {
                    ui.vertical(|ui| {
                        let (rect, resp) = ui.allocate_exact_size(vec2(180.0, 101.0), Sense::click());
                        paint_thumb(ui, rect, &v, false);
                        if resp.clicked() { self.open_detail(&v.id); }
                        ui.add_sized([180.0, 30.0], egui::Label::new(RichText::new(trunc(&v.title, 26)).size(11.0).strong().color(pal.ink)));
                    });
                }
            });
        });
        ui.add_space(8.0);
        ui.label(RichText::new("Untukmu").size(15.0).strong().color(pal.ink));
        let feed: Vec<Video> = self.videos().into_iter().filter(|v| self.category == "Semua" || v.category == self.category).collect();
        if self.state.settings.data_saver {
            for v in feed {
                let (clicked, _, _) = item_row(ui, &v, pal, &[], None, None);
                if clicked { self.open_detail(&v.id); }
            }
        } else {
            for v in feed {
                self.video_card(ui, &v, pal);
            }
        }
        ui.add_space(10.0);
        ui.label(RichText::new("Semua video diputar lewat Brave. Iklan dan pelacak diblokir di sumbernya.").size(11.0).color(pal.muted));
    }

    fn video_card(&mut self, ui: &mut egui::Ui, v: &Video, pal: &Pal) {
        Frame::none().fill(pal.surface).rounding(16.0).inner_margin(Margin::same(10.0)).show(ui, |ui| {
            let w = ui.available_width();
            let (rect, resp) = ui.allocate_exact_size(vec2(w, w * 9.0 / 16.0), Sense::click());
            paint_thumb(ui, rect, v, false);
            if resp.clicked() { self.open_detail(&v.id); }
            ui.add_space(6.0);
            if ui.link(RichText::new(trunc(&v.title, 78)).size(14.0).strong().color(pal.ink)).clicked() { self.open_detail(&v.id); }
            ui.label(RichText::new(format!("{} • {} x ditonton • {}", v.channel, fmt_views(v.views), v.age)).size(11.0).color(pal.muted));
        });
        ui.add_space(10.0);
    }

    fn search_screen(&mut self, ui: &mut egui::Ui, pal: &Pal) {
        ui.horizontal(|ui| {
            let w = (ui.available_width() - 100.0).max(120.0);
            let resp = ui.add(egui::TextEdit::singleline(&mut self.search).hint_text("Cari video atau channel...").desired_width(w));
            if resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                let q = self.search.clone();
                self.add_recent(&q);
            }
            if ui.button("Tambah link").clicked() {
                self.link_input.clear();
                self.dialog = Some(Dialog::AddLink);
            }
        });
        if !self.state.recents.is_empty() {
            let recents = self.state.recents.clone();
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new("Terakhir:").size(10.0).color(pal.muted));
                for r in recents {
                    if ui.add(egui::Button::new(RichText::new(&r).size(11.0).color(pal.ink)).fill(pal.surface2)).clicked() { self.search = r.clone(); }
                    if ui.add(egui::Button::new(RichText::new("x").size(11.0).color(pal.muted)).fill(pal.surface2)).clicked() {
                        self.state.recents.retain(|x| x != &r);
                        self.mark();
                    }
                }
            });
        }
        let results = self.search_results();
        ui.horizontal(|ui| {
            egui::ComboBox::from_id_source("sort").selected_text(self.sort.clone()).show_ui(ui, |ui| {
                for s in ["Relevansi", "Terpopuler", "Terbaru", "Durasi"] {
                    ui.selectable_value(&mut self.sort, s.to_string(), s);
                }
            });
            ui.label(RichText::new(format!("{} hasil", results.len())).size(11.0).color(pal.muted));
        });
        ui.add_space(4.0);
        if results.is_empty() {
            empty_state(ui, pal, "Tidak ada hasil", "Coba kata kunci lain, atau tambahkan video langsung dari link YouTube.");
        } else {
            for v in results {
                let (clicked, _, _) = item_row(ui, &v, pal, &[], None, None);
                if clicked { self.open_detail(&v.id); }
            }
        }
    }

    fn subs_screen(&mut self, ui: &mut egui::Ui, pal: &Pal) {
        ui.label(RichText::new("Langganan").size(18.0).strong().color(pal.ink));
        ui.label(RichText::new("Ketuk channel untuk berlangganan / berhenti.").size(11.0).color(pal.muted));
        let channels = self.channels();
        egui::ScrollArea::horizontal().show(ui, |ui| {
            ui.horizontal(|ui| {
                for (cid, name, count) in &channels {
                    let subbed = self.state.subs.get(cid).copied().unwrap_or(false);
                    let label = format!("{} ({})", name, count);
                    if ui.add(egui::Button::new(RichText::new(label).size(11.0).color(if subbed { GREEN } else { pal.ink })).fill(pal.surface2)).clicked() {
                        self.toggle_sub(cid);
                    }
                }
            });
        });
        ui.add_space(6.0);
        let feed: Vec<Video> = self.videos().into_iter().filter(|v| self.state.subs.get(&v.channel_id).copied().unwrap_or(false)).collect();
        if feed.is_empty() {
            empty_state(ui, pal, "Belum ada langganan", "Berlangganan channel di atas agar videonya muncul di sini.");
            if ui.button("Jelajahi video").clicked() { self.tab = Tab::Home; }
        } else {
            for v in feed {
                let (clicked, _, _) = item_row(ui, &v, pal, &[], None, None);
                if clicked { self.open_detail(&v.id); }
            }
        }
    }

    fn library_screen(&mut self, ui: &mut egui::Ui, pal: &Pal) {
        ui.label(RichText::new("Pustaka").size(18.0).strong().color(pal.ink));
        ui.horizontal(|ui| {
            let tabs = [(LibraryTab::History, "Riwayat"), (LibraryTab::Playlists, "Playlist"), (LibraryTab::Likes, "Disukai"), (LibraryTab::WatchLater, "Tonton nanti")];
            for (lt, label) in tabs {
                let on = self.lib_tab == lt;
                if ui.add(egui::Button::new(RichText::new(label).size(12.0).strong().color(if on { WHITE } else { pal.muted })).fill(if on { RED } else { pal.surface2 })).clicked() {
                    self.lib_tab = lt;
                    self.selected.clear();
                }
            }
        });
        ui.add_space(6.0);
        match self.lib_tab {
            LibraryTab::History => self.history_tab(ui, pal),
            LibraryTab::Playlists => self.playlists_tab(ui, pal),
            LibraryTab::Likes => self.likes_tab(ui, pal),
            LibraryTab::WatchLater => self.watch_later_tab(ui, pal),
        }
    }

    fn history_tab(&mut self, ui: &mut egui::Ui, pal: &Pal) {
        ui.horizontal(|ui| {
            ui.label(RichText::new(format!("{} video", self.state.history.len())).size(11.0).color(pal.muted));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if !self.state.history.is_empty() && ui.button("Bersihkan").clicked() {
                    let old = self.state.history.clone();
                    self.state.history.clear();
                    self.mark();
                    self.toast_undo("Riwayat dibersihkan", UndoAction::RestoreHistory(old));
                }
            });
        });
        if self.state.history.is_empty() {
            empty_state(ui, pal, "Riwayat kosong", "Video yang kamu putar akan tercatat di sini.");
            return;
        }
        let items = self.state.history.clone();
        for h in items {
            let v = match self.video(&h.id) { Some(v) => v, None => continue };
            let sub = ago(h.at);
            let (clicked, act, _) = item_row(ui, &v, pal, &["Putar", "Hapus"], None, Some(&sub));
            if clicked { self.open_detail(&v.id); break; }
            match act.as_deref() {
                Some("Putar") => { self.play(ui, &v.id); break; }
                Some("Hapus") => {
                    let old = self.state.history.clone();
                    self.state.history.retain(|x| x.id != v.id);
                    self.mark();
                    self.toast_undo("Dihapus dari riwayat", UndoAction::RestoreHistory(old));
                    break;
                }
                _ => {}
            }
        }
    }

    fn playlists_tab(&mut self, ui: &mut egui::Ui, pal: &Pal) {
        if ui.add(egui::Button::new(RichText::new("+ Playlist baru").size(13.0).strong().color(WHITE)).fill(RED)).clicked() {
            self.new_playlist_name.clear();
            self.dialog = Some(Dialog::NewPlaylist);
        }
        ui.add_space(6.0);
        if self.state.playlists.is_empty() {
            empty_state(ui, pal, "Belum ada playlist", "Buat playlist untuk mengelompokkan video favoritmu.");
            return;
        }
        let pls = self.state.playlists.clone();
        for pl in pls {
            let mut open = false;
            let mut del = false;
            Frame::none().fill(pal.surface).rounding(14.0).inner_margin(Margin::same(10.0)).show(ui, |ui| {
                ui.horizontal(|ui| {
                    let (rect, _) = ui.allocate_exact_size(vec2(56.0, 56.0), Sense::hover());
                    let p = ui.painter_at(rect);
                    let hue = [RED, BLUE, VIOLET, AMBER, GREEN, CYAN, MAGENTA][(pl.ids.len()) % 7];
                    p.rect_filled(rect, Rounding::same(12.0), hue);
                    p.text(rect.center(), Align2::CENTER_CENTER, pl.ids.len().to_string(), FontId::proportional(18.0), WHITE);
                    ui.vertical(|ui| {
                        ui.label(RichText::new(trunc(&pl.name, 30)).size(14.0).strong().color(pal.ink));
                        ui.label(RichText::new(format!("{} video", pl.ids.len())).size(11.0).color(pal.muted));
                        ui.horizontal(|ui| {
                            if ui.button("Buka").clicked() { open = true; }
                            if ui.button("Hapus").clicked() { del = true; }
                        });
                    });
                });
            });
            ui.add_space(8.0);
            if open { self.playlist_detail = Some(pl.id.clone()); self.selected.clear(); break; }
            if del {
                self.state.playlists.retain(|p| p.id != pl.id);
                self.mark();
                self.toast_undo("Playlist dihapus", UndoAction::RestorePlaylist(pl.clone()));
                break;
            }
        }
    }

    fn likes_tab(&mut self, ui: &mut egui::Ui, pal: &Pal) {
        let ids: Vec<String> = self.state.likes.iter().filter(|(_, v)| **v).map(|(k, _)| k.clone()).collect();
        if ids.is_empty() {
            empty_state(ui, pal, "Belum ada video disukai", "Ketuk Suka pada video untuk menyimpannya di sini.");
            return;
        }
        for id in ids {
            let v = match self.video(&id) { Some(v) => v, None => continue };
            let (clicked, act, _) = item_row(ui, &v, pal, &["Putar", "Hapus"], None, None);
            if clicked { self.open_detail(&v.id); break; }
            match act.as_deref() {
                Some("Putar") => { self.play(ui, &v.id); break; }
                Some("Hapus") => { self.toggle_like(&v.id); break; }
                _ => {}
            }
        }
    }

    fn watch_later_tab(&mut self, ui: &mut egui::Ui, pal: &Pal) {
        let ids = self.state.watch_later.clone();
        if ids.is_empty() {
            empty_state(ui, pal, "Tonton nanti kosong", "Simpan video untuk ditonton nanti dari halaman detail.");
            return;
        }
        for id in ids {
            let v = match self.video(&id) { Some(v) => v, None => continue };
            let (clicked, act, _) = item_row(ui, &v, pal, &["Putar", "Hapus"], None, None);
            if clicked { self.open_detail(&v.id); break; }
            match act.as_deref() {
                Some("Putar") => { self.play(ui, &v.id); break; }
                Some("Hapus") => { self.toggle_watch_later(&v.id); break; }
                _ => {}
            }
        }
    }

    fn playlist_screen(&mut self, ui: &mut egui::Ui, pid: &str, pal: &Pal) {
        let pl = match self.state.playlists.iter().find(|p| p.id == pid).cloned() {
            Some(p) => p,
            None => { self.playlist_detail = None; return; }
        };
        if ui.button("Kembali").clicked() { self.playlist_detail = None; self.selected.clear(); return; }
        ui.horizontal(|ui| {
            ui.label(RichText::new(trunc(&pl.name, 28)).size(18.0).strong().color(pal.ink));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui.button("Hapus").clicked() {
                    self.state.playlists.retain(|p| p.id != pid);
                    self.playlist_detail = None;
                    self.mark();
                    self.toast_undo("Playlist dihapus", UndoAction::RestorePlaylist(pl.clone()));
                }
                if ui.button("Ganti nama").clicked() {
                    self.rename_name = pl.name.clone();
                    self.dialog = Some(Dialog::RenamePlaylist(pl.id.clone()));
                }
            });
        });
        ui.label(RichText::new(format!("{} video", pl.ids.len())).size(11.0).color(pal.muted));
        ui.horizontal(|ui| {
            if ui.add(egui::Button::new(RichText::new("Putar semua").size(12.0).strong().color(WHITE)).fill(RED)).clicked() {
                if let Some(first) = pl.ids.first().cloned() { self.play(ui, &first); }
            }
            egui::ComboBox::from_id_source("libsort").selected_text(self.lib_sort.clone()).show_ui(ui, |ui| {
                for s in ["Urutan", "Judul", "Durasi", "Terpopuler"] {
                    ui.selectable_value(&mut self.lib_sort, s.to_string(), s);
                }
            });
        });
        ui.add(egui::TextEdit::singleline(&mut self.lib_search).hint_text("Cari di playlist...").desired_width(ui.available_width()));
        ui.horizontal(|ui| {
            let all_selected = !pl.ids.is_empty() && pl.ids.iter().all(|id| self.selected.contains(id));
            if ui.button(if all_selected { "Batal pilih" } else { "Pilih semua" }).clicked() {
                if all_selected { self.selected.clear(); } else { self.selected = pl.ids.iter().cloned().collect(); }
            }
            if !self.selected.is_empty() {
                ui.label(RichText::new(format!("{} dipilih", self.selected.len())).size(11.0).color(pal.muted));
                if ui.button("Hapus terpilih").clicked() {
                    let old = pl.clone();
                    if let Some(p) = self.playlist_mut(pid) { p.ids.retain(|id| !self.selected.contains(id)); }
                    self.selected.clear();
                    self.mark();
                    self.toast_undo("Video dihapus dari playlist", UndoAction::RestorePlaylist(old));
                    return;
                }
            }
        });
        ui.add_space(4.0);
        let q = self.lib_search.trim().to_lowercase();
        let mut ids: Vec<String> = pl.ids.iter().filter(|id| {
            if q.is_empty() { return true; }
            match self.video(id) { Some(v) => v.title.to_lowercase().contains(&q) || v.channel.to_lowercase().contains(&q), None => false }
        }).cloned().collect();
        if self.lib_sort != "Urutan" {
            let mut vids: Vec<Video> = ids.iter().filter_map(|id| self.video(id)).collect();
            match self.lib_sort.as_str() {
                "Judul" => vids.sort_by(|a, b| a.title.cmp(&b.title)),
                "Durasi" => vids.sort_by(|a, b| b.duration.cmp(&a.duration)),
                "Terpopuler" => vids.sort_by(|a, b| b.views.cmp(&a.views)),
                _ => {}
            }
            ids = vids.into_iter().map(|v| v.id).collect();
        }
        if ids.is_empty() {
            empty_state(ui, pal, "Playlist kosong", "Tambahkan video dari halaman detail dengan tombol Simpan.");
            return;
        }
        let reorder = self.lib_sort == "Urutan";
        for id in ids {
            let v = match self.video(&id) { Some(v) => v, None => continue };
            let actions: &[&str] = if reorder { &["Putar", "Atas", "Bawah", "Hapus"] } else { &["Putar", "Hapus"] };
            let checked = Some(self.selected.contains(&id));
            let (clicked, act, newcheck) = item_row(ui, &v, pal, actions, checked, None);
            if let Some(c) = newcheck {
                if c { self.selected.insert(id.clone()); } else { self.selected.remove(&id); }
            }
            if clicked { self.open_detail(&id); break; }
            let mut acted = true;
            match act.as_deref() {
                Some("Putar") => self.play(ui, &id),
                Some("Hapus") => {
                    let old = pl.clone();
                    if let Some(p) = self.playlist_mut(pid) { p.ids.retain(|x| x != &id); }
                    self.mark();
                    self.toast_undo("Video dihapus", UndoAction::RestorePlaylist(old));
                }
                Some("Atas") => {
                    if let Some(p) = self.playlist_mut(pid) {
                        if let Some(pos) = p.ids.iter().position(|x| x == &id) {
                            if pos > 0 { p.ids.swap(pos, pos - 1); self.mark(); }
                        }
                    }
                }
                Some("Bawah") => {
                    if let Some(p) = self.playlist_mut(pid) {
                        if let Some(pos) = p.ids.iter().position(|x| x == &id) {
                            if pos + 1 < p.ids.len() { p.ids.swap(pos, pos + 1); self.mark(); }
                        }
                    }
                }
                _ => acted = false,
            }
            if acted { break; }
        }
    }

    fn detail_screen(&mut self, ui: &mut egui::Ui, id: &str, pal: &Pal) {
        let v = match self.video(id) {
            Some(v) => v,
            None => {
                ui.label("Video tidak ditemukan.");
                if ui.button("Kembali").clicked() { self.detail = None; }
                return;
            }
        };
        if ui.button("Kembali").clicked() { self.detail = None; return; }
        let w = ui.available_width();
        let (rect, _) = ui.allocate_exact_size(vec2(w, w * 9.0 / 16.0), Sense::hover());
        paint_thumb(ui, rect, &v, false);
        ui.add_space(8.0);
        ui.label(RichText::new(&v.title).size(17.0).strong().color(pal.ink));
        ui.label(RichText::new(format!("{} x ditonton • {} • {}", fmt_views(v.views), v.age, v.category)).size(11.0).color(pal.muted));
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            let (r, _) = ui.allocate_exact_size(vec2(34.0, 34.0), Sense::hover());
            let p = ui.painter_at(r);
            p.circle_filled(r.center(), 17.0, Color32::from_rgb(v.color[0], v.color[1], v.color[2]));
            p.text(r.center(), Align2::CENTER_CENTER, initials(&v.channel), FontId::proportional(13.0), WHITE);
            ui.vertical(|ui| {
                ui.label(RichText::new(&v.channel).size(13.0).strong().color(pal.ink));
                ui.label(RichText::new("Channel YouTube").size(10.0).color(pal.muted));
            });
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let subbed = self.state.subs.get(&v.channel_id).copied().unwrap_or(false);
                let label = if subbed { "Tersubscribe" } else { "Subscribe" };
                if ui.add(egui::Button::new(RichText::new(label).size(12.0).strong().color(if subbed { pal.muted } else { WHITE })).fill(if subbed { pal.surface2 } else { RED })).clicked() {
                    self.toggle_sub(&v.channel_id);
                }
            });
        });
        ui.add_space(6.0);
        egui::ScrollArea::horizontal().show(ui, |ui| {
            ui.horizontal(|ui| {
                if ui.add(egui::Button::new(RichText::new("Putar di Brave").size(13.0).strong().color(WHITE)).fill(RED)).clicked() { self.play(ui, &v.id); }
                let liked = self.state.likes.get(&v.id).copied().unwrap_or(false);
                if ui.add(egui::Button::new(RichText::new(if liked { "Disukai" } else { "Suka" }).size(12.0).color(if liked { RED } else { pal.ink })).fill(pal.surface2)).clicked() { self.toggle_like(&v.id); }
                let wl = self.state.watch_later.contains(&v.id);
                if ui.add(egui::Button::new(RichText::new(if wl { "Tersimpan" } else { "Tonton nanti" }).size(12.0).color(if wl { GREEN } else { pal.ink })).fill(pal.surface2)).clicked() { self.toggle_watch_later(&v.id); }
                if ui.add(egui::Button::new(RichText::new("Simpan").size(12.0).color(pal.ink)).fill(pal.surface2)).clicked() { self.dialog = Some(Dialog::SaveToPlaylist(v.id.clone())); }
                if ui.add(egui::Button::new(RichText::new("Salin link").size(12.0).color(pal.ink)).fill(pal.surface2)).clicked() {
                    let url = format!("https://www.youtube.com/watch?v={}", v.id);
                    ui.ctx().output_mut(|o| o.copied_text = url);
                    self.toast("Link disalin ke clipboard");
                }
            });
        });
        ui.add_space(6.0);
        ui.collapsing(RichText::new("Deskripsi").size(12.0).color(pal.ink), |ui| {
            ui.label(RichText::new(format!("Channel: {}", v.channel)).size(12.0).color(pal.muted));
            ui.label(RichText::new(format!("Durasi: {}", fmt_dur(v.duration))).size(12.0).color(pal.muted));
            ui.label(RichText::new("Diputar melalui Brave dengan Shields aktif: iklan, pelacak, dan fingerprinting diblokir sebelum halaman dimuat.").size(12.0).color(pal.muted));
        });
        if self.state.settings.sponsor {
            if self.pending.contains(&format!("s:{}", v.id)) {
                ui.label(RichText::new("Memuat info SponsorBlock...").size(11.0).color(pal.muted));
            } else if let Some(segs) = self.segments.get(&v.id) {
                if segs.is_empty() {
                    ui.label(RichText::new("SponsorBlock: tidak ada segmen sponsor untuk video ini.").size(11.0).color(pal.muted));
                } else {
                    ui.label(RichText::new(format!("SponsorBlock: {} segmen komunitas", segs.len())).size(12.0).strong().color(VIOLET));
                    for s in segs {
                        ui.label(RichText::new(format!("{}: {} - {} ({} dtk)", s.category, fmt_dur(s.start as u32), fmt_dur(s.end as u32), (s.end - s.start) as u32)).size(11.0).color(pal.muted));
                    }
                }
            }
        }
        ui.add_space(8.0);
        ui.label(RichText::new("Video terkait").size(14.0).strong().color(pal.ink));
        let related: Vec<Video> = self.videos().into_iter().filter(|x| x.category == v.category && x.id != v.id).take(6).collect();
        for x in related {
            let (clicked, _, _) = item_row(ui, &x, pal, &[], None, None);
            if clicked { self.open_detail(&x.id); break; }
        }
    }

    fn dialogs(&mut self, ctx: &egui::Context, pal: &Pal) {
        let dlg = match self.dialog.clone() { Some(d) => d, None => return };
        let mut open = true;
        let mut close = false;
        match dlg {
            Dialog::NewPlaylist => {
                egui::Window::new("Playlist baru").open(&mut open).collapsible(false).resizable(false).default_width(300.0).show(ctx, |ui| {
                    ui.label(RichText::new("Nama playlist").size(12.0).color(pal.muted));
                    ui.text_edit_singleline(&mut self.new_playlist_name);
                    ui.horizontal(|ui| {
                        if ui.add(egui::Button::new(RichText::new("Buat").strong().color(WHITE)).fill(RED)).clicked() {
                            let name = self.new_playlist_name.trim().to_string();
                            if !name.is_empty() { self.create_playlist(&name); self.new_playlist_name.clear(); close = true; }
                        }
                        if ui.button("Batal").clicked() { close = true; }
                    });
                });
            }
            Dialog::RenamePlaylist(pid) => {
                egui::Window::new("Ganti nama playlist").open(&mut open).collapsible(false).resizable(false).default_width(300.0).show(ctx, |ui| {
                    ui.text_edit_singleline(&mut self.rename_name);
                    ui.horizontal(|ui| {
                        if ui.add(egui::Button::new(RichText::new("Simpan").strong().color(WHITE)).fill(RED)).clicked() {
                            let name = self.rename_name.trim().to_string();
                            if !name.is_empty() {
                                if let Some(p) = self.playlist_mut(&pid) { p.name = name; }
                                self.mark();
                                close = true;
                            }
                        }
                        if ui.button("Batal").clicked() { close = true; }
                    });
                });
            }
            Dialog::AddLink => {
                egui::Window::new("Tambah dari link YouTube").open(&mut open).collapsible(false).resizable(false).default_width(320.0).show(ctx, |ui| {
                    ui.label(RichText::new("Tempel link watch, youtu.be, atau shorts.").size(12.0).color(pal.muted));
                    ui.text_edit_singleline(&mut self.link_input);
                    ui.horizontal(|ui| {
                        if ui.add(egui::Button::new(RichText::new("Tambah").strong().color(WHITE)).fill(RED)).clicked() {
                            let raw = self.link_input.trim().to_string();
                            match parse_yt_id(&raw) {
                                Some(id) => {
                                    if self.video(&id).is_some() {
                                        self.open_detail(&id);
                                        self.toast("Video sudah ada di katalog");
                                        close = true;
                                    } else {
                                        let key = format!("o:{}", id);
                                        if !self.pending.contains(&key) {
                                            self.pending.insert(key);
                                            let tx = self.tx.clone();
                                            let id2 = id.clone();
                                            std::thread::spawn(move || {
                                                let r = sponsor::fetch_oembed(&id2);
                                                let _ = tx.send(AsyncResult::OEmbed { id: id2, result: r });
                                            });
                                            self.toast("Mengambil info video...");
                                            close = true;
                                        }
                                    }
                                }
                                None => self.toast("Link YouTube tidak valid"),
                            }
                        }
                        if ui.button("Batal").clicked() { close = true; }
                    });
                });
            }
            Dialog::ConfirmClear => {
                egui::Window::new("Reset data aplikasi").open(&mut open).collapsible(false).resizable(false).default_width(300.0).show(ctx, |ui| {
                    ui.label(RichText::new("Hapus semua data lokal? Playlist, riwayat, suka, langganan, dan statistik akan direset.").size(12.0).color(pal.ink));
                    ui.horizontal(|ui| {
                        if ui.add(egui::Button::new(RichText::new("Hapus semua").strong().color(WHITE)).fill(RED)).clicked() {
                            let old = self.state.clone();
                            self.state = State::default();
                            self.mark();
                            close = true;
                            self.toast_undo("Data direset", UndoAction::RestoreState(Box::new(old)));
                        }
                        if ui.button("Batal").clicked() { close = true; }
                    });
                });
            }
            Dialog::SaveToPlaylist(vid) => {
                egui::Window::new("Simpan ke playlist").open(&mut open).collapsible(false).resizable(false).default_width(300.0).show(ctx, |ui| {
                    let pls = self.state.playlists.clone();
                    if pls.is_empty() {
                        ui.label(RichText::new("Belum ada playlist. Buat di bawah.").size(12.0).color(pal.muted));
                    }
                    for pl in pls {
                        let already = pl.ids.contains(&vid);
                        let label = if already { format!("{} (tersimpan)", pl.name) } else { pl.name.clone() };
                        if ui.button(label).clicked() {
                            if !already {
                                if let Some(p) = self.playlist_mut(&pl.id) { p.ids.push(vid.clone()); }
                                self.mark();
                                self.toast(format!("Disimpan ke {}", pl.name));
                            }
                            close = true;
                        }
                    }
                    ui.separator();
                    ui.label(RichText::new("Playlist baru").size(12.0).color(pal.muted));
                    ui.text_edit_singleline(&mut self.new_playlist_name);
                    if ui.add(egui::Button::new(RichText::new("Buat & simpan").strong().color(WHITE)).fill(RED)).clicked() {
                        let name = self.new_playlist_name.trim().to_string();
                        if !name.is_empty() {
                            let id = self.create_playlist(&name);
                            if let Some(p) = self.playlist_mut(&id) { p.ids.push(vid.clone()); }
                            self.new_playlist_name.clear();
                            self.mark();
                            close = true;
                        }
                    }
                });
            }
            Dialog::Stats => {
                egui::Window::new("Statistik perlindungan").open(&mut open).collapsible(false).resizable(true).default_width(340.0).show(ctx, |ui| {
                    let s = self.state.stats.clone();
                    egui::Grid::new("stats").num_columns(2).spacing([24.0, 8.0]).show(ui, |ui| {
                        ui.label(RichText::new("Video dibuka").size(12.0).color(pal.muted));
                        ui.label(RichText::new(s.opened.to_string()).size(14.0).strong().color(pal.ink));
                        ui.end_row();
                        ui.label(RichText::new("Iklan diblokir (total)").size(12.0).color(pal.muted));
                        ui.label(RichText::new(s.blocked.to_string()).size(14.0).strong().color(GREEN));
                        ui.end_row();
                        ui.label(RichText::new("Iklan diblokir hari ini").size(12.0).color(pal.muted));
                        ui.label(RichText::new(s.today.to_string()).size(14.0).strong().color(GREEN));
                        ui.end_row();
                        ui.label(RichText::new("Waktu dihemat").size(12.0).color(pal.muted));
                        ui.label(RichText::new(format!("{} menit", s.time_saved_min)).size(14.0).strong().color(pal.ink));
                        ui.end_row();
                        ui.label(RichText::new("Data dihemat").size(12.0).color(pal.muted));
                        ui.label(RichText::new(format!("{} MB", s.data_saved_mb)).size(14.0).strong().color(pal.ink));
                        ui.end_row();
                    });
                    ui.separator();
                    ui.label(RichText::new("Iklan diblokir per kategori").size(12.0).strong().color(pal.ink));
                    let mut cats: Vec<(String, u64)> = s.per_category.iter().map(|(k, v)| (k.clone(), *v)).collect();
                    cats.sort_by(|a, b| b.1.cmp(&a.1));
                    if cats.is_empty() {
                        ui.label(RichText::new("Belum ada data. Putar video pertamamu.").size(11.0).color(pal.muted));
                    } else {
                        let w = ui.available_width();
                        let h = 140.0;
                        let (rect, _) = ui.allocate_exact_size(vec2(w, h), Sense::hover());
                        let p = ui.painter_at(rect);
                        let max = cats.iter().map(|(_, v)| *v).max().unwrap_or(1).max(1);
                        let n = cats.len().min(7);
                        let bw = (w - 20.0) / n as f32;
                        let hues = [RED, BLUE, VIOLET, AMBER, GREEN, CYAN, MAGENTA];
                        for (i, (cat, val)) in cats.iter().take(n).enumerate() {
                            let bh = (*val as f32 / max as f32) * (h - 34.0);
                            let x = rect.left() + 10.0 + i as f32 * bw;
                            let bar = Rect::from_min_max(pos2(x + 4.0, rect.bottom() - 18.0 - bh), pos2(x + bw - 4.0, rect.bottom() - 18.0));
                            p.rect_filled(bar, Rounding::same(4.0), hues[i % 7]);
                            p.text(pos2(bar.center().x, bar.top() - 8.0), Align2::CENTER_CENTER, val.to_string(), FontId::proportional(10.0), pal.ink);
                            p.text(pos2(bar.center().x, rect.bottom() - 8.0), Align2::CENTER_CENTER, trunc(cat, 8), FontId::proportional(9.0), pal.muted);
                        }
                    }
                    ui.label(RichText::new("Estimasi berdasarkan jumlah video yang dibuka lewat Brave Shields.").size(10.0).color(pal.muted));
                });
            }
            Dialog::Settings => {
                egui::Window::new("Setelan").open(&mut open).collapsible(false).resizable(false).default_width(320.0).show(ctx, |ui| {
                    if ui.checkbox(&mut self.state.settings.block_ads, "Mode tanpa iklan (Brave)").changed() { self.mark(); }
                    ui.label(RichText::new("Jika mati, video dibuka di browser biasa.").size(10.0).color(pal.muted));
                    if ui.checkbox(&mut self.state.settings.sponsor, "Info SponsorBlock (komunitas)").changed() { self.mark(); }
                    if ui.checkbox(&mut self.state.settings.background, "Buka browser otomatis").changed() { self.mark(); }
                    ui.label(RichText::new("Jika mati, link hanya disalin ke clipboard.").size(10.0).color(pal.muted));
                    if ui.checkbox(&mut self.state.settings.data_saver, "Hemat data (tampilan ringkas)").changed() { self.mark(); }
                    ui.separator();
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Tema").size(12.0).color(pal.ink));
                        egui::ComboBox::from_id_source("theme").selected_text(if self.state.settings.theme == "light" { "Terang" } else { "Gelap" }).show_ui(ui, |ui| {
                            if ui.selectable_label(self.state.settings.theme == "dark", "Gelap").clicked() { self.state.settings.theme = "dark".into(); self.mark(); }
                            if ui.selectable_label(self.state.settings.theme == "light", "Terang").clicked() { self.state.settings.theme = "light".into(); self.mark(); }
                        });
                    });
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Browser").size(12.0).color(pal.ink));
                        let cur = match self.state.settings.brave_package.as_str() {
                            "com.brave.browser" => "Brave",
                            "com.brave.browser_beta" => "Brave Beta",
                            "com.brave.browser_nightly" => "Brave Nightly",
                            "default" => "Browser default",
                            _ => "Otomatis",
                        };
                        egui::ComboBox::from_id_source("brave").selected_text(cur).show_ui(ui, |ui| {
                            for (val, label) in [("auto", "Otomatis"), ("com.brave.browser", "Brave"), ("com.brave.browser_beta", "Brave Beta"), ("com.brave.browser_nightly", "Brave Nightly"), ("default", "Browser default")] {
                                if ui.selectable_label(self.state.settings.brave_package == val, label).clicked() {
                                    self.state.settings.brave_package = val.to_string();
                                    self.mark();
                                }
                            }
                        });
                    });
                    ui.separator();
                    if ui.add(egui::Button::new(RichText::new("Reset semua data").size(12.0).color(RED)).fill(pal.surface2)).clicked() {
                        self.dialog = Some(Dialog::ConfirmClear);
                    }
                    ui.label(RichText::new("NoIklan v1.0.0 • Rust + egui • Android NativeActivity").size(10.0).color(pal.muted));
                });
            }
        }
        if !open || close { self.dialog = None; }
    }

    fn snack_area(&mut self, ctx: &egui::Context, pal: &Pal) {
        let s = match self.snack.clone() { Some(s) => s, None => return };
        let pos = ctx.screen_rect().center_bottom() + vec2(0.0, -70.0);
        egui::Area::new(egui::Id::new("snack")).fixed_pos(pos).order(egui::Order::Foreground).show(ctx, |ui| {
            Frame::none().fill(pal.surface2).rounding(12.0).stroke(Stroke::new(1.0, pal.line)).inner_margin(Margin::same(10.0)).show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(&s.msg).size(12.0).color(pal.ink));
                    if let Some(u) = s.undo.clone() {
                        if ui.add(egui::Button::new(RichText::new("Urungkan").size(12.0).strong().color(GREEN)).fill(pal.surface)).clicked() {
                            self.apply_undo(u);
                            self.snack = None;
                        }
                    }
                });
            });
        });
    }
}

impl eframe::App for NoIklanApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_async(ctx);
        if let Some(s) = &self.snack {
            if Instant::now() > s.until { self.snack = None; }
        }
        self.apply_style(ctx);
        let pal = self.pal();
        egui::TopBottomPanel::top("top").frame(Frame::none().fill(pal.bg).inner_margin(Margin::same(10.0))).show(ctx, |ui| self.top_bar(ui, &pal));
        egui::TopBottomPanel::bottom("tabs").frame(Frame::none().fill(pal.surface).inner_margin(Margin::same(8.0))).show(ctx, |ui| self.tab_bar(ui, &pal));
        egui::CentralPanel::default().frame(Frame::none().fill(pal.bg).inner_margin(Margin::same(10.0))).show(ctx, |ui| {
            egui::ScrollArea::vertical().auto_shrink([false; 2]).show(ui, |ui| {
                if let Some(id) = self.detail.clone() {
                    self.detail_screen(ui, &id, &pal);
                } else if let Some(pid) = self.playlist_detail.clone() {
                    self.playlist_screen(ui, &pid, &pal);
                } else {
                    match self.tab {
                        Tab::Home => self.home_screen(ui, &pal),
                        Tab::Search => self.search_screen(ui, &pal),
                        Tab::Subs => self.subs_screen(ui, &pal),
                        Tab::Library => self.library_screen(ui, &pal),
                    }
                }
            });
        });
        self.dialogs(ctx, &pal);
        self.snack_area(ctx, &pal);
        self.autosave();
    }
}

fn paint_thumb(ui: &egui::Ui, rect: Rect, v: &Video, small: bool) {
    let p = ui.painter_at(rect);
    let base = Color32::from_rgb(v.color[0], v.color[1], v.color[2]);
    let dark = Color32::from_rgb((v.color[0] as f32 * 0.30) as u8, (v.color[1] as f32 * 0.30) as u8, (v.color[2] as f32 * 0.30) as u8);
    let mid = Color32::from_rgb((v.color[0] as f32 * 0.55) as u8, (v.color[1] as f32 * 0.55) as u8, (v.color[2] as f32 * 0.55) as u8);
    p.rect_filled(rect, Rounding::same(if small { 8.0 } else { 12.0 }), dark);
    let n = if small { 3 } else { 6 };
    for i in 0..n {
        let x = rect.left() + (i as f32) * rect.width() / (n as f32);
        p.line_segment([pos2(x, rect.bottom()), pos2(x + rect.width() * 0.35, rect.top())], Stroke::new(if small { 2.0 } else { 3.0 }, mid));
    }
    p.circle_filled(pos2(rect.right() - rect.width() * 0.18, rect.top() + rect.height() * 0.30), rect.height() * 0.28, base);
    p.circle_filled(pos2(rect.left() + rect.width() * 0.14, rect.bottom() - rect.height() * 0.24), rect.height() * 0.16, mid);
    let c = rect.center();
    let r = rect.height() * 0.13;
    p.add(egui::Shape::convex_polygon(vec![pos2(c.x - r * 0.6, c.y - r), pos2(c.x - r * 0.6, c.y + r), pos2(c.x + r, c.y)], Color32::from_white_alpha(230), Stroke::NONE));
    if v.duration > 0 {
        let bw = if small { 40.0 } else { 52.0 };
        let bh = if small { 16.0 } else { 20.0 };
        let br = Rect::from_min_size(pos2(rect.right() - bw - 6.0, rect.bottom() - bh - 6.0), vec2(bw, bh));
        p.rect_filled(br, Rounding::same(5.0), Color32::from_black_alpha(190));
        p.text(br.center(), Align2::CENTER_CENTER, fmt_dur(v.duration), FontId::proportional(if small { 9.0 } else { 11.0 }), WHITE);
    }
    if !small {
        let br = Rect::from_min_size(pos2(rect.left() + 6.0, rect.top() + 6.0), vec2(96.0, 18.0));
        p.rect_filled(br, Rounding::same(9.0), Color32::from_black_alpha(150));
        p.text(br.center(), Align2::CENTER_CENTER, "TANPA IKLAN", FontId::proportional(9.0), GREEN);
    }
}

fn item_row(ui: &mut egui::Ui, v: &Video, pal: &Pal, actions: &[&str], checked: Option<bool>, sub: Option<&str>) -> (bool, Option<String>, Option<bool>) {
    let mut clicked = false;
    let mut act: Option<String> = None;
    let mut newcheck: Option<bool> = None;
    Frame::none().fill(pal.surface).rounding(12.0).inner_margin(Margin::same(8.0)).show(ui, |ui| {
        ui.horizontal(|ui| {
            if let Some(c) = checked {
                let mut cc = c;
                if ui.checkbox(&mut cc, "").changed() { newcheck = Some(cc); }
            }
            let (rect, resp) = ui.allocate_exact_size(vec2(108.0, 61.0), Sense::click());
            paint_thumb(ui, rect, v, true);
            if resp.clicked() { clicked = true; }
            let rest = (ui.available_width() - 4.0).max(80.0);
            ui.vertical(|ui| {
                ui.set_max_width(rest);
                if ui.link(RichText::new(trunc(&v.title, 42)).size(12.0).strong().color(pal.ink)).clicked() { clicked = true; }
                let meta = match sub { Some(s) => s.to_string(), None => format!("{} • {}", v.channel, fmt_dur(v.duration)) };
                ui.label(RichText::new(meta).size(10.0).color(pal.muted));
                if !actions.is_empty() {
                    ui.horizontal(|ui| {
                        for a in actions {
                            if ui.add(egui::Button::new(RichText::new(*a).size(10.0).color(pal.ink)).fill(pal.surface2)).clicked() {
                                act = Some((*a).to_string());
                            }
                        }
                    });
                }
            });
        });
    });
    ui.add_space(6.0);
    (clicked, act, newcheck)
}

fn empty_state(ui: &mut egui::Ui, pal: &Pal, title: &str, body: &str) {
    ui.add_space(20.0);
    Frame::none().fill(pal.surface).rounding(16.0).inner_margin(Margin::same(18.0)).show(ui, |ui| {
        ui.vertical_centered(|ui| {
            ui.label(RichText::new(title).size(15.0).strong().color(pal.ink));
            ui.add_space(4.0);
            ui.label(RichText::new(body).size(12.0).color(pal.muted));
        });
    });
    ui.add_space(10.0);
}

fn fmt_dur(secs: u32) -> String {
    if secs == 0 { return "--:--".into(); }
    let h = secs / 3600;
    let m = (secs % 3600) / 60;
    let s = secs % 60;
    if h > 0 { format!("{}:{:02}:{:02}", h, m, s) } else { format!("{}:{:02}", m, s) }
}

fn fmt_views(n: u64) -> String {
    if n >= 1_000_000_000 { format!("{:.1} M", n as f64 / 1e9) }
    else if n >= 1_000_000 { format!("{:.1} jt", n as f64 / 1e6) }
    else if n >= 1_000 { format!("{:.0} rb", n as f64 / 1e3) }
    else { n.to_string() }
}

fn trunc(s: &str, n: usize) -> String {
    if s.chars().count() > n {
        let t: String = s.chars().take(n.saturating_sub(2)).collect();
        format!("{}..", t)
    } else {
        s.to_string()
    }
}

fn initials(s: &str) -> String {
    let mut out = String::new();
    for w in s.split_whitespace().take(2) {
        if let Some(c) = w.chars().next() { out.extend(c.to_uppercase()); }
    }
    if out.is_empty() { out.push('?'); }
    out
}

fn ago(at: u64) -> String {
    let d = now().saturating_sub(at);
    if d < 60 { "baru saja".into() }
    else if d < 3600 { format!("{} mnt lalu", d / 60) }
    else if d < 86400 { format!("{} jam lalu", d / 3600) }
    else { format!("{} hari lalu", d / 86400) }
}

fn parse_yt_id(s: &str) -> Option<String> {
    let s = s.trim();
    let valid = |c: char| c.is_ascii_alphanumeric() || c == '-' || c == '_';
    if s.len() == 11 && s.chars().all(valid) { return Some(s.to_string()); }
    let cand = if let Some(pos) = s.find("youtu.be/") { &s[pos + 9..] }
    else if let Some(pos) = s.find("v=") { &s[pos + 2..] }
    else if let Some(pos) = s.find("shorts/") { &s[pos + 7..] }
    else if let Some(pos) = s.find("embed/") { &s[pos + 6..] }
    else if let Some(pos) = s.find("live/") { &s[pos + 5..] }
    else { return None; };
    let id: String = cand.chars().take_while(|c| valid(*c)).collect();
    if id.len() == 11 { Some(id) } else { None }
}
