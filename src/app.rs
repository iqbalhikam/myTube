use crate::icons;
use crate::model::*;
use crate::{brave, catalog, sponsor, store, youtube, DATA_DIR};
use egui::{
    pos2, vec2, Align, Align2, Color32, FontId, Frame, Layout, Margin, Rect, Response, RichText, Rounding, Sense, Shape, Stroke,
    TextureHandle, TextureOptions,
};
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::time::{Duration, Instant};

const RED: Color32 = Color32::from_rgb(255, 0, 0);

#[derive(Clone, Copy)]
struct Pal {
    bg: Color32,
    surface: Color32,
    surface2: Color32,
    ink: Color32,
    muted: Color32,
    line: Color32,
    blue: Color32,
}

fn palette(light: bool) -> Pal {
    if light {
        Pal {
            bg: Color32::from_rgb(255, 255, 255),
            surface: Color32::from_rgb(242, 242, 242),
            surface2: Color32::from_rgb(229, 229, 229),
            ink: Color32::from_rgb(15, 15, 15),
            muted: Color32::from_rgb(96, 96, 96),
            line: Color32::from_rgb(224, 224, 224),
            blue: Color32::from_rgb(6, 95, 212),
        }
    } else {
        Pal {
            bg: Color32::from_rgb(15, 15, 15),
            surface: Color32::from_rgb(33, 33, 33),
            surface2: Color32::from_rgb(39, 39, 39),
            ink: Color32::from_rgb(241, 241, 241),
            muted: Color32::from_rgb(170, 170, 170),
            line: Color32::from_rgb(48, 48, 48),
            blue: Color32::from_rgb(62, 166, 255),
        }
    }
}

enum AsyncResult {
    Trending(Result<Vec<Video>, String>),
    Search { q: String, result: Result<Vec<Video>, String> },
    OEmbed { id: String, result: Result<Video, String> },
    Sponsor { id: String, result: Result<Vec<Segment>, String> },
    Thumb { url: String, w: usize, h: usize, rgba: Vec<u8> },
    ThumbFailed { url: String },
}

#[derive(Clone, PartialEq)]
enum Screen {
    Tabs,
    Search,
    Detail(String),
    Playlist(String),
    Settings,
    Stats,
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
    screen: Screen,
    back_to: Option<Screen>,
    chip: String,
    feed: Vec<Video>,
    feed_loading: bool,
    feed_err: Option<String>,
    search: String,
    query: String,
    results: Vec<Video>,
    search_loading: bool,
    search_err: Option<String>,
    focus_search: bool,
    dialog: Option<Dialog>,
    snack: Option<Snack>,
    data_path: PathBuf,
    dirty: bool,
    last_save: Instant,
    tx: Sender<AsyncResult>,
    rx: Receiver<AsyncResult>,
    pending: HashSet<String>,
    segments: HashMap<String, Vec<Segment>>,
    thumbs: HashMap<String, TextureHandle>,
    thumb_order: VecDeque<String>,
    thumb_pending: HashSet<String>,
    new_playlist_name: String,
    rename_name: String,
    link_input: String,
    selected: HashSet<String>,
    lib_search: String,
    lib_sort: String,
    safe_top: f32,
    safe_bottom: f32,
}

impl NoIklanApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        icons::install(&cc.egui_ctx);
        let data_path = DATA_DIR.get().cloned().unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
        let state = store::load(&data_path);
        let (tx, rx) = channel();
        let mut app = Self {
            state,
            catalog: catalog::videos(),
            tab: Tab::Home,
            lib_tab: LibraryTab::History,
            screen: Screen::Tabs,
            back_to: None,
            chip: "Semua".into(),
            feed: Vec::new(),
            feed_loading: false,
            feed_err: None,
            search: String::new(),
            query: String::new(),
            results: Vec::new(),
            search_loading: false,
            search_err: None,
            focus_search: false,
            dialog: None,
            snack: None,
            data_path,
            dirty: false,
            last_save: Instant::now(),
            tx,
            rx,
            pending: HashSet::new(),
            segments: HashMap::new(),
            thumbs: HashMap::new(),
            thumb_order: VecDeque::new(),
            thumb_pending: HashSet::new(),
            new_playlist_name: String::new(),
            rename_name: String::new(),
            link_input: String::new(),
            selected: HashSet::new(),
            lib_search: String::new(),
            lib_sort: "Urutan".into(),
            safe_top: 0.0,
            safe_bottom: 0.0,
        };
        app.start_feed();
        app
    }

    fn pal(&self) -> Pal {
        palette(self.state.settings.theme == "light")
    }

    fn videos(&self) -> Vec<Video> {
        let mut v = self.catalog.clone();
        v.extend(self.state.custom_videos.iter().cloned());
        v
    }

    fn video(&self, id: &str) -> Option<Video> {
        self.catalog
            .iter()
            .find(|v| v.id == id)
            .cloned()
            .or_else(|| self.state.custom_videos.iter().find(|v| v.id == id).cloned())
            .or_else(|| self.feed.iter().find(|v| v.id == id).cloned())
            .or_else(|| self.results.iter().find(|v| v.id == id).cloned())
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

    fn start_feed(&mut self) {
        if self.feed_loading {
            return;
        }
        self.feed_loading = true;
        self.feed_err = None;
        let tx = self.tx.clone();
        std::thread::spawn(move || {
            let _ = tx.send(AsyncResult::Trending(youtube::trending()));
        });
    }

    fn start_search(&mut self, q: &str) {
        let q = q.trim().to_string();
        if q.is_empty() {
            return;
        }
        self.search = q.clone();
        self.query = q.clone();
        self.screen = Screen::Search;
        self.search_loading = true;
        self.search_err = None;
        self.results.clear();
        self.add_recent(&q);
        let tx = self.tx.clone();
        let q2 = q.clone();
        std::thread::spawn(move || {
            let result = youtube::search(&q2);
            let _ = tx.send(AsyncResult::Search { q: q2, result });
        });
    }

    fn open_search(&mut self) {
        self.screen = Screen::Search;
        self.focus_search = true;
        if self.query.is_empty() && !self.results.is_empty() {
            self.query = self.search.clone();
        }
    }

    fn open_detail(&mut self, id: &str) {
        if self.screen != Screen::Detail(id.to_string()) {
            self.back_to = Some(self.screen.clone());
        }
        self.screen = Screen::Detail(id.to_string());
        if self.state.settings.sponsor && !self.segments.contains_key(id) {
            let key = format!("s:{}", id);
            if self.pending.insert(key) {
                let tx = self.tx.clone();
                let id2 = id.to_string();
                std::thread::spawn(move || {
                    let r = sponsor::fetch_segments(&id2);
                    let _ = tx.send(AsyncResult::Sponsor { id: id2, result: r });
                });
            }
        }
    }

    fn back(&mut self) {
        self.screen = self.back_to.take().unwrap_or(Screen::Tabs);
        self.selected.clear();
    }

    fn play(&mut self, ui: &egui::Ui, id: &str) {
        let v = match self.video(id) {
            Some(v) => v,
            None => return,
        };
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
                        self.toast("Diputar di Brave — iklan diblokir.");
                    } else if pkg == "default" {
                        self.toast("Brave tidak terbuka, dibuka di browser default.");
                    } else {
                        self.toast("Diputar di browser.");
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
            self.toast_undo("Dihapus dari video yang disukai", UndoAction::RestoreLike(id.to_string()));
        } else {
            self.state.likes.insert(id.to_string(), true);
            self.mark();
            self.toast("Ditambahkan ke video yang disukai");
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
        if q.is_empty() {
            return;
        }
        self.state.recents.retain(|x| x != &q);
        self.state.recents.insert(0, q);
        self.state.recents.truncate(10);
        self.mark();
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

    fn apply_undo(&mut self, u: UndoAction) {
        match u {
            UndoAction::RestoreHistory(h) => self.state.history = h,
            UndoAction::RestorePlaylist(pl) => {
                if let Some(p) = self.playlist_mut(&pl.id) {
                    *p = pl;
                } else {
                    self.state.playlists.push(pl);
                }
            }
            UndoAction::RestoreLike(id) => {
                self.state.likes.insert(id, true);
            }
            UndoAction::RestoreWatchLater(id) => {
                if !self.state.watch_later.contains(&id) {
                    self.state.watch_later.push(id);
                }
            }
            UndoAction::RestoreState(s) => self.state = *s,
        }
        self.mark();
    }

    fn poll_async(&mut self, ctx: &egui::Context) {
        while let Ok(res) = self.rx.try_recv() {
            match res {
                AsyncResult::Trending(result) => {
                    self.feed_loading = false;
                    match result {
                        Ok(v) if !v.is_empty() => {
                            self.feed = v;
                            self.feed_err = None;
                        }
                        Ok(_) => {
                            self.feed = catalog::videos();
                            self.feed_err = Some("YouTube tidak mengembalikan video.".into());
                        }
                        Err(e) => {
                            self.feed = catalog::videos();
                            self.feed_err = Some(e);
                        }
                    }
                }
                AsyncResult::Search { q, result } => {
                    if q == self.query {
                        self.search_loading = false;
                        match result {
                            Ok(v) => {
                                self.results = v;
                                self.search_err = None;
                            }
                            Err(e) => {
                                self.results.clear();
                                self.search_err = Some(e);
                            }
                        }
                    }
                }
                AsyncResult::OEmbed { id, result } => {
                    self.pending.remove(&format!("o:{}", id));
                    match result {
                        Ok(v) => {
                            if self.video(&id).is_none() {
                                self.state.custom_videos.push(v);
                                self.mark();
                            }
                            self.open_detail(&id);
                            self.toast("Video ditambahkan");
                        }
                        Err(e) => self.toast(format!("Gagal mengambil info video: {}", e)),
                    }
                }
                AsyncResult::Sponsor { id, result } => {
                    self.pending.remove(&format!("s:{}", id));
                    self.segments.insert(id, result.unwrap_or_default());
                }
                AsyncResult::Thumb { url, w, h, rgba } => {
                    self.thumb_pending.remove(&url);
                    let img = egui::ColorImage::from_rgba_unmultiplied([w, h], &rgba);
                    let tex = ctx.load_texture(format!("t{}", url), img, TextureOptions::LINEAR);
                    self.thumbs.insert(url.clone(), tex);
                    self.thumb_order.push_back(url);
                    while self.thumb_order.len() > 180 {
                        if let Some(old) = self.thumb_order.pop_front() {
                            self.thumbs.remove(&old);
                        }
                    }
                }
                AsyncResult::ThumbFailed { url } => {
                    self.thumb_pending.remove(&url);
                }
            }
        }
    }

    fn autosave(&mut self) {
        if self.dirty && self.last_save.elapsed() > Duration::from_millis(800) {
            let _ = store::save(&self.data_path, &self.state);
            self.dirty = false;
            self.last_save = Instant::now();
        }
    }

    fn thumb(&mut self, url: &str) -> Option<TextureHandle> {
        if let Some(t) = self.thumbs.get(url) {
            return Some(t.clone());
        }
        if self.thumb_pending.insert(url.to_string()) {
            let tx = self.tx.clone();
            let url2 = url.to_string();
            std::thread::spawn(move || {
                match youtube::fetch_thumb(&url2) {
                    Ok((w, h, rgba)) => {
                        let _ = tx.send(AsyncResult::Thumb { url: url2, w, h, rgba });
                    }
                    Err(_) => {
                        let _ = tx.send(AsyncResult::ThumbFailed { url: url2 });
                    }
                }
            });
        }
        None
    }

    fn draw_thumb(&mut self, ui: &mut egui::Ui, rect: Rect, v: &Video, radius: f32) {
        let url = youtube::thumb_url(&v.id);
        let mut painted = false;
        if let Some(tex) = self.thumb(&url) {
            let img = egui::Image::new(egui::load::SizedTexture::new(tex.id(), tex.size_vec2()))
                .rounding(Rounding::same(radius))
                .fit_to_exact_size(rect.size());
            ui.put(rect, img);
            painted = true;
        }
        let p = ui.painter_at(rect);
        if !painted {
            let base = Color32::from_rgb(v.color[0], v.color[1], v.color[2]);
            let dark = Color32::from_rgb((v.color[0] as f32 * 0.22) as u8, (v.color[1] as f32 * 0.22) as u8, (v.color[2] as f32 * 0.22) as u8);
            p.rect_filled(rect, Rounding::same(radius), dark);
            p.text(rect.center(), Align2::CENTER_CENTER, icons::PLAY_CIRCLE, FontId::new(rect.height() * 0.22, icons::icon_family()), Color32::from_rgba_unmultiplied(base.r(), base.g(), base.b(), 90));
        }
        if v.live {
            let br = Rect::from_min_size(pos2(rect.right() - 52.0, rect.bottom() - 26.0), vec2(44.0, 18.0));
            p.rect_filled(br, Rounding::same(4.0), RED);
            p.text(br.center(), Align2::CENTER_CENTER, "LIVE", FontId::new(11.0, icons::medium_family()), Color32::WHITE);
        } else if v.duration > 0 {
            let label = fmt_dur(v.duration);
            let w = 7.0 * label.len() as f32 + 12.0;
            let br = Rect::from_min_size(pos2(rect.right() - w - 6.0, rect.bottom() - 26.0), vec2(w, 20.0));
            p.rect_filled(br, Rounding::same(4.0), Color32::from_black_alpha(200));
            p.text(br.center(), Align2::CENTER_CENTER, label, FontId::proportional(12.0), Color32::WHITE);
        }
    }

    fn draw_avatar(&mut self, ui: &mut egui::Ui, rect: Rect, name: &str, color: [u8; 3], url: Option<&str>) {
        if let Some(u) = url {
            if let Some(tex) = self.thumb(u) {
                let img = egui::Image::new(egui::load::SizedTexture::new(tex.id(), tex.size_vec2()))
                    .rounding(Rounding::same(rect.width() / 2.0))
                    .fit_to_exact_size(rect.size());
                ui.put(rect, img);
                return;
            }
        }
        let p = ui.painter_at(rect);
        p.circle_filled(rect.center(), rect.width() / 2.0, Color32::from_rgb(color[0], color[1], color[2]));
        p.text(rect.center(), Align2::CENTER_CENTER, initials(name), FontId::proportional(rect.width() * 0.36), Color32::WHITE);
    }

    fn apply_style(&self, ctx: &egui::Context) {
        let pal = self.pal();
        let mut v = if self.state.settings.theme == "light" { egui::Visuals::light() } else { egui::Visuals::dark() };
        v.panel_fill = pal.bg;
        v.window_fill = pal.surface;
        v.extreme_bg_color = pal.surface2;
        v.faint_bg_color = pal.surface;
        v.selection.bg_fill = pal.blue;
        v.selection.stroke = Stroke::new(1.0_f32, Color32::WHITE);
        v.hyperlink_color = pal.blue;
        v.window_rounding = Rounding::same(16.0);
        v.widgets.noninteractive.bg_fill = pal.surface;
        v.widgets.inactive.bg_fill = pal.surface2;
        v.widgets.hovered.bg_fill = pal.surface2;
        v.widgets.active.bg_fill = pal.surface2;
        v.widgets.inactive.rounding = Rounding::same(8.0);
        v.widgets.hovered.rounding = Rounding::same(8.0);
        ctx.set_visuals(v);
        ctx.style_mut(|s| {
            s.spacing.item_spacing = vec2(8.0, 6.0);
            s.spacing.button_padding = vec2(12.0, 7.0);
        });
    }

    fn related(&self, v: &Video) -> Vec<Video> {
        let mut out: Vec<Video> = Vec::new();
        let mut seen = HashSet::new();
        for x in self.results.iter().chain(self.feed.iter()).chain(self.catalog.iter()) {
            if x.id != v.id && seen.insert(x.id.clone()) {
                out.push(x.clone());
            }
            if out.len() >= 12 {
                break;
            }
        }
        out
    }

    // ---------- navigasi ----------

    fn top_home(&mut self, ctx: &egui::Context, pal: &Pal) {
        egui::TopBottomPanel::top("top")
            .exact_height(52.0 + self.safe_top)
            .frame(Frame::none().fill(pal.bg).inner_margin(Margin { left: 14.0, right: 6.0, top: 10.0 + self.safe_top, bottom: 4.0 }))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    let (r, _) = ui.allocate_exact_size(vec2(32.0, 22.0), Sense::hover());
                    let p = ui.painter_at(r);
                    p.rect_filled(r, Rounding::same(5.0), RED);
                    p.add(Shape::convex_polygon(
                        vec![pos2(r.center().x - 4.0, r.center().y - 6.0), pos2(r.center().x - 4.0, r.center().y + 6.0), pos2(r.center().x + 7.0, r.center().y)],
                        Color32::WHITE,
                        Stroke::NONE,
                    ));
                    ui.label(RichText::new("NoIklan").size(20.0).family(icons::medium_family()).color(pal.ink));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let (ar, r2) = ui.allocate_exact_size(vec2(28.0, 28.0), Sense::click());
                        self.draw_avatar(ui, ar, "N", [255, 45, 77], None);
                        if r2.clicked() {
                            self.tab = Tab::Library;
                            self.screen = Screen::Tabs;
                        }
                        if icon_btn(ui, icons::BELL, 22.0, pal.ink).clicked() {
                            self.toast("Tidak ada notifikasi baru");
                        }
                        if icon_btn(ui, icons::CAST, 22.0, pal.ink).clicked() {
                            self.toast("Cast belum didukung");
                        }
                        if icon_btn(ui, icons::SEARCH, 22.0, pal.ink).clicked() {
                            self.open_search();
                        }
                    });
                });
            });
    }

    fn top_back(&mut self, ctx: &egui::Context, pal: &Pal, title: &str, subtitle: Option<&str>) {
        egui::TopBottomPanel::top("top")
            .exact_height(52.0 + self.safe_top)
            .frame(Frame::none().fill(pal.bg).inner_margin(Margin { left: 6.0, right: 8.0, top: 10.0 + self.safe_top, bottom: 4.0 }))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    if icon_btn(ui, icons::ARROW_BACK, 22.0, pal.ink).clicked() {
                        self.back();
                    }
                    ui.vertical(|ui| {
                        ui.label(RichText::new(title).size(16.0).family(icons::medium_family()).color(pal.ink));
                        if let Some(s) = subtitle {
                            ui.label(RichText::new(s).size(10.0).color(pal.muted));
                        }
                    });
                });
            });
    }

    fn top_search(&mut self, ctx: &egui::Context, pal: &Pal) {
        egui::TopBottomPanel::top("top")
            .exact_height(56.0 + self.safe_top)
            .frame(Frame::none().fill(pal.bg).inner_margin(Margin { left: 6.0, right: 8.0, top: 10.0 + self.safe_top, bottom: 6.0 }))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    if icon_btn(ui, icons::ARROW_BACK, 22.0, pal.ink).clicked() {
                        self.back();
                    }
                    let field_w = (ui.available_width() - 80.0).max(120.0);
                    let resp = ui.add(
                        egui::TextEdit::singleline(&mut self.search)
                            .hint_text("Telusuri YouTube")
                            .desired_width(field_w)
                            .font(FontId::proportional(15.0))
                            .margin(Margin::symmetric(12.0, 8.0)),
                    );
                    if self.focus_search {
                        resp.request_focus();
                        self.focus_search = false;
                    }
                    let submitted = resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                    if submitted {
                        let q = self.search.clone();
                        self.start_search(&q);
                    }
                    if icon_btn(ui, icons::MIC, 20.0, pal.ink).clicked() {
                        self.toast("Input suara belum didukung");
                    }
                    if icon_btn(ui, icons::SEARCH, 20.0, pal.ink).clicked() {
                        let q = self.search.clone();
                        self.start_search(&q);
                    }
                });
            });
    }

    fn bottom_nav(&mut self, ctx: &egui::Context, pal: &Pal) {
        egui::TopBottomPanel::bottom("nav")
            .exact_height(58.0 + self.safe_bottom)
            .frame(Frame::none().fill(pal.bg).inner_margin(Margin { left: 0.0, right: 0.0, top: 4.0, bottom: 2.0 + self.safe_bottom }))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 0.0;
                    let w = ui.available_width() / 5.0;
                    let home = self.tab == Tab::Home && self.screen == Screen::Tabs;
                    if nav_item(ui, icons::HOME, "Beranda", home, pal, w).clicked() {
                        self.tab = Tab::Home;
                        self.screen = Screen::Tabs;
                        self.selected.clear();
                    }
                    let shorts = self.tab == Tab::Shorts && self.screen == Screen::Tabs;
                    if nav_item(ui, icons::SHORTS, "Shorts", shorts, pal, w).clicked() {
                        self.tab = Tab::Shorts;
                        self.screen = Screen::Tabs;
                        self.selected.clear();
                    }
                    if plus_item(ui, pal, w).clicked() {
                        self.link_input.clear();
                        self.dialog = Some(Dialog::AddLink);
                    }
                    let subs = self.tab == Tab::Subs && self.screen == Screen::Tabs;
                    if nav_item(ui, icons::SUBS, "Langganan", subs, pal, w).clicked() {
                        self.tab = Tab::Subs;
                        self.screen = Screen::Tabs;
                        self.selected.clear();
                    }
                    let lib = self.tab == Tab::Library && self.screen == Screen::Tabs;
                    if nav_item(ui, icons::PERSON, "Kamu", lib, pal, w).clicked() {
                        self.tab = Tab::Library;
                        self.screen = Screen::Tabs;
                        self.selected.clear();
                    }
                });
            });
    }

    // ---------- konten tab ----------

    fn body(&mut self, ui: &mut egui::Ui, pal: &Pal) {
        match self.tab {
            Tab::Home => self.home(ui, pal),
            Tab::Shorts => self.shorts(ui, pal),
            Tab::Subs => self.subs(ui, pal),
            Tab::Library => self.library(ui, pal),
        }
    }

    fn chips(&mut self, ui: &mut egui::Ui, pal: &Pal) {
        egui::ScrollArea::horizontal().id_source("chips").show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 8.0;
                let labels = ["Semua", "Musik", "Gaming", "Berita", "Podcast", "Langsung", "Olahraga", "Teknologi", "Komedi", "Film"];
                for label in labels {
                    let active = self.chip == label;
                    if chip(ui, label, active, pal).clicked() {
                        if label == "Semua" {
                            self.chip = "Semua".into();
                            self.tab = Tab::Home;
                        } else {
                            self.chip = label.into();
                            self.start_search(label);
                        }
                    }
                }
            });
        });
    }

    fn home(&mut self, ui: &mut egui::Ui, pal: &Pal) {
        ui.add_space(6.0);
        self.chips(ui, pal);
        ui.add_space(10.0);
        if let Some(h) = self.state.history.first().cloned() {
            if let Some(v) = self.video(&h.id) {
                let mut open = false;
                Frame::none().fill(pal.surface).rounding(Rounding::same(12.0)).inner_margin(Margin::same(10.0)).show(ui, |ui| {
                    ui.horizontal(|ui| {
                        let (rect, resp) = ui.allocate_exact_size(vec2(132.0, 74.0), Sense::click());
                        self.draw_thumb(ui, rect, &v, 8.0);
                        if resp.clicked() {
                            open = true;
                        }
                        ui.vertical(|ui| {
                            ui.label(RichText::new("Lanjutkan menonton").size(10.0).family(icons::medium_family()).color(pal.ink));
                            ui.add(egui::Label::new(RichText::new(trunc(&v.title, 48)).size(13.0).color(pal.ink)).wrap(true));
                            ui.label(RichText::new(&v.channel).size(11.0).color(pal.muted));
                        });
                    });
                });
                if open {
                    self.open_detail(&v.id);
                }
                ui.add_space(12.0);
            }
        }
        if let Some(err) = self.feed_err.clone() {
            Frame::none().fill(pal.surface).rounding(Rounding::same(12.0)).inner_margin(Margin::same(12.0)).show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(icons::WIFI_OFF).family(icons::icon_family()).size(18.0).color(pal.muted));
                    ui.vertical(|ui| {
                        ui.label(RichText::new("Tidak bisa memuat beranda YouTube").size(13.0).family(icons::medium_family()).color(pal.ink));
                        ui.label(RichText::new(trunc(&err, 90)).size(11.0).color(pal.muted));
                    });
                });
                if ui.add(egui::Button::new(RichText::new("Coba lagi").size(12.0).family(icons::medium_family()).color(Color32::BLACK)).fill(pal.ink).rounding(Rounding::same(18.0))).clicked() {
                    self.start_feed();
                }
            });
            ui.add_space(12.0);
        }
        if self.feed.is_empty() && self.feed_loading {
            for _ in 0..3 {
                skeleton(ui, pal);
            }
        } else {
            let feed = self.feed.clone();
            for v in &feed {
                self.video_card(ui, v, pal);
            }
            if feed.is_empty() {
                empty_state(ui, pal, "Belum ada video", "Tarik tombol Coba lagi setelah koneksi kembali.");
            }
        }
        ui.add_space(16.0);
        ui.label(RichText::new("Video diputar lewat Brave dengan pemblokiran iklan.").size(10.0).color(pal.muted));
        ui.add_space(8.0);
    }

    fn video_card(&mut self, ui: &mut egui::Ui, v: &Video, pal: &Pal) {
        let w = ui.available_width();
        let h = w * 9.0 / 16.0;
        let (rect, resp) = ui.allocate_exact_size(vec2(w, h), Sense::click());
        self.draw_thumb(ui, rect, v, 12.0);
        if resp.clicked() {
            self.open_detail(&v.id);
        }
        ui.add_space(8.0);
        let mut open = false;
        let mut more = false;
        ui.horizontal(|ui| {
            let (ar, _r) = ui.allocate_exact_size(vec2(36.0, 36.0), Sense::hover());
            self.draw_avatar(ui, ar, &v.channel, v.color, v.avatar.as_deref());
            let text_w = (w - 36.0 - 40.0).max(100.0);
            ui.allocate_ui_with_layout(vec2(text_w, 58.0), Layout::top_down(Align::Min), |ui| {
                let title = ui.add(egui::Label::new(RichText::new(trunc(&v.title, 90)).size(14.0).family(icons::medium_family()).color(pal.ink)).wrap(true));
                if title.clicked() {
                    open = true;
                }
                ui.label(RichText::new(format!("{} • {}", v.channel, meta_line(v))).size(12.0).color(pal.muted));
            });
            ui.with_layout(Layout::right_to_left(Align::Min), |ui| {
                if icon_btn(ui, icons::MORE_VERT, 18.0, pal.muted).clicked() {
                    more = true;
                }
            });
        });
        if open {
            self.open_detail(&v.id);
        }
        if more {
            self.dialog = Some(Dialog::SaveToPlaylist(v.id.clone()));
        }
        ui.add_space(18.0);
    }

    fn shorts(&mut self, ui: &mut egui::Ui, pal: &Pal) {
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            ui.label(RichText::new(icons::SHORTS).family(icons::icon_family()).size(22.0).color(RED));
            ui.label(RichText::new("Shorts").size(18.0).family(icons::medium_family()).color(pal.ink));
        });
        ui.add_space(6.0);
        let list: Vec<Video> = if !self.feed.is_empty() { self.feed.clone() } else { self.videos() };
        let h = (ui.available_height() - 10.0).max(420.0);
        for v in list.iter().take(12) {
            let w = ui.available_width();
            let (rect, resp) = ui.allocate_exact_size(vec2(w, h), Sense::click());
            let p = ui.painter_at(rect);
            p.rect_filled(rect, Rounding::same(12.0), Color32::BLACK);
            let img_h = w * 9.0 / 16.0;
            let img_rect = Rect::from_center_size(rect.center(), vec2(w, img_h));
            self.draw_thumb(ui, img_rect, v, 0.0);
            if resp.clicked() {
                self.open_detail(&v.id);
                break;
            }
            let (ar, _) = ui.allocate_exact_size(vec2(0.0, 0.0), Sense::hover());
            let _ = ar;
            let p = ui.painter_at(rect);
            let grad = Rect::from_min_max(pos2(rect.left(), rect.bottom() - 150.0), rect.max);
            p.rect_filled(grad, Rounding::same(12.0), Color32::from_black_alpha(110));
            let av = Rect::from_min_size(pos2(rect.left() + 14.0, rect.bottom() - 96.0), vec2(32.0, 32.0));
            self.draw_avatar(ui, av, &v.channel, v.color, v.avatar.as_deref());
            p.text(pos2(rect.left() + 54.0, rect.bottom() - 88.0), Align2::LEFT_TOP, trunc(&v.channel, 24), FontId::new(13.0, icons::medium_family()), Color32::WHITE);
            p.text(pos2(rect.left() + 14.0, rect.bottom() - 54.0), Align2::LEFT_TOP, trunc(&v.title, 64), FontId::proportional(12.0), Color32::WHITE);
            let x = rect.right() - 24.0;
            p.text(pos2(x, rect.bottom() - 150.0), Align2::CENTER_CENTER, icons::THUMB_UP, FontId::new(24.0, icons::icon_family()), Color32::WHITE);
            p.text(pos2(x, rect.bottom() - 108.0), Align2::CENTER_CENTER, icons::THUMB_DOWN, FontId::new(24.0, icons::icon_family()), Color32::WHITE);
            p.text(pos2(x, rect.bottom() - 66.0), Align2::CENTER_CENTER, icons::COMMENT, FontId::new(24.0, icons::icon_family()), Color32::WHITE);
            p.text(pos2(x, rect.bottom() - 24.0), Align2::CENTER_CENTER, icons::SHARE, FontId::new(24.0, icons::icon_family()), Color32::WHITE);
            ui.add_space(10.0);
        }
    }

    fn subs(&mut self, ui: &mut egui::Ui, pal: &Pal) {
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            ui.label(RichText::new("Langganan").size(20.0).family(icons::medium_family()).color(pal.ink));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if icon_btn(ui, icons::SETTINGS, 20.0, pal.ink).clicked() {
                    self.screen = Screen::Settings;
                }
            });
        });
        ui.add_space(4.0);
        let channels = self.channels();
        egui::ScrollArea::horizontal().id_source("subs").show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 14.0;
                for (cid, name, _count) in &channels {
                    let subbed = self.state.subs.get(cid).copied().unwrap_or(false);
                    let mut clicked = false;
                    ui.vertical(|ui| {
                        ui.set_width(64.0);
                        let (r, resp) = ui.allocate_exact_size(vec2(56.0, 56.0), Sense::click());
                        self.draw_avatar(ui, r, name, [124, 92, 255], None);
                        if subbed {
                            ui.painter_at(r).circle_stroke(r.center(), 28.0, Stroke::new(2.0_f32, RED));
                        }
                        if resp.clicked() {
                            clicked = true;
                        }
                        ui.add(egui::Label::new(RichText::new(trunc(name, 10)).size(10.0).color(pal.muted)).truncate(true));
                    });
                    if clicked {
                        self.toggle_sub(cid);
                    }
                }
            });
        });
        ui.add_space(10.0);
        let feed: Vec<Video> = self.videos().into_iter().filter(|v| self.state.subs.get(&v.channel_id).copied().unwrap_or(false)).collect();
        if feed.is_empty() {
            empty_state(ui, pal, "Belum ada langganan", "Ketuk channel di atas untuk berlangganan.");
        } else {
            for v in &feed {
                self.video_row(ui, v, pal);
            }
        }
        ui.add_space(10.0);
    }

    fn library(&mut self, ui: &mut egui::Ui, pal: &Pal) {
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            let (ar, _) = ui.allocate_exact_size(vec2(56.0, 56.0), Sense::hover());
            self.draw_avatar(ui, ar, "N", [255, 45, 77], None);
            ui.vertical(|ui| {
                ui.label(RichText::new("Kamu").size(20.0).family(icons::medium_family()).color(pal.ink));
                ui.label(RichText::new("@noiklan • Rust + egui").size(11.0).color(pal.muted));
            });
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if icon_btn(ui, icons::SETTINGS, 20.0, pal.ink).clicked() {
                    self.screen = Screen::Settings;
                }
            });
        });
        ui.add_space(10.0);
        egui::ScrollArea::horizontal().id_source("libchips").show(ui, |ui| {
            ui.horizontal(|ui| {
                for (lt, label) in [(LibraryTab::History, "Riwayat"), (LibraryTab::Playlists, "Playlist"), (LibraryTab::Likes, "Disukai"), (LibraryTab::WatchLater, "Tonton nanti")] {
                    if chip(ui, label, self.lib_tab == lt, pal).clicked() {
                        self.lib_tab = lt;
                        self.selected.clear();
                    }
                }
            });
        });
        ui.add_space(10.0);
        match self.lib_tab {
            LibraryTab::History => self.history_tab(ui, pal),
            LibraryTab::Playlists => self.playlists_tab(ui, pal),
            LibraryTab::Likes => self.likes_tab(ui, pal),
            LibraryTab::WatchLater => self.watch_later_tab(ui, pal),
        }
        ui.add_space(16.0);
        let rows: [(&str, &str); 2] = [(icons::INSIGHTS, "Statistik"), (icons::SETTINGS, "Setelan")];
        for (icon, label) in rows {
            let mut go = false;
            let resp = Frame::none().inner_margin(Margin { left: 2.0, right: 2.0, top: 12.0, bottom: 12.0 }).show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(icon).family(icons::icon_family()).size(22.0).color(pal.ink));
                    ui.label(RichText::new(label).size(14.0).color(pal.ink));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.label(RichText::new(icons::CHEVRON_RIGHT).family(icons::icon_family()).size(20.0).color(pal.muted));
                    });
                });
            }).response;
            if resp.interact(Sense::click()).clicked() {
                go = true;
                if label == "Statistik" {
                    self.screen = Screen::Stats;
                } else {
                    self.screen = Screen::Settings;
                }
            }
            if go {
                break;
            }
        }
    }

    fn history_tab(&mut self, ui: &mut egui::Ui, pal: &Pal) {
        ui.horizontal(|ui| {
            ui.label(RichText::new(format!("{} video", self.state.history.len())).size(11.0).color(pal.muted));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if !self.state.history.is_empty() && ui.add(egui::Button::new(RichText::new("Hapus riwayat").size(11.0).color(pal.ink)).frame(false)).clicked() {
                    let old = self.state.history.clone();
                    self.state.history.clear();
                    self.mark();
                    self.toast_undo("Riwayat dibersihkan", UndoAction::RestoreHistory(old));
                }
            });
        });
        if self.state.history.is_empty() {
            empty_state(ui, pal, "Riwayat kosong", "Video yang kamu tonton akan muncul di sini.");
            return;
        }
        let items = self.state.history.clone();
        for h in items {
            let v = match self.video(&h.id) {
                Some(v) => v,
                None => continue,
            };
            let mut clicked = false;
            let mut play = false;
            ui.horizontal(|ui| {
                let (rect, resp) = ui.allocate_exact_size(vec2(150.0, 84.0), Sense::click());
                self.draw_thumb(ui, rect, &v, 8.0);
                if resp.clicked() {
                    clicked = true;
                }
                ui.vertical(|ui| {
                    ui.add(egui::Label::new(RichText::new(trunc(&v.title, 52)).size(13.0).family(icons::medium_family()).color(pal.ink)).wrap(true));
                    ui.label(RichText::new(format!("{} • {}", v.channel, ago(h.at))).size(11.0).color(pal.muted));
                    ui.horizontal(|ui| {
                        if icon_btn(ui, icons::PLAY, 16.0, pal.ink).clicked() {
                            play = true;
                        }
                        if icon_btn(ui, icons::DELETE, 16.0, pal.muted).clicked() {
                            let old = self.state.history.clone();
                            self.state.history.retain(|x| x.id != v.id);
                            self.mark();
                            self.toast_undo("Dihapus dari riwayat", UndoAction::RestoreHistory(old));
                        }
                    });
                });
            });
            if clicked || play {
                if play {
                    self.play(ui, &v.id);
                } else {
                    self.open_detail(&v.id);
                }
                break;
            }
            ui.add_space(10.0);
        }
    }

    fn playlists_tab(&mut self, ui: &mut egui::Ui, pal: &Pal) {
        let mut new = false;
        let resp = Frame::none().inner_margin(Margin { left: 2.0, right: 2.0, top: 10.0, bottom: 10.0 }).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new(icons::ADD).family(icons::icon_family()).size(22.0).color(pal.ink));
                ui.label(RichText::new("Playlist baru").size(14.0).color(pal.ink));
            });
        }).response;
        if resp.interact(Sense::click()).clicked() {
            new = true;
        }
        if new {
            self.new_playlist_name.clear();
            self.dialog = Some(Dialog::NewPlaylist);
        }
        ui.add_space(6.0);
        if self.state.playlists.is_empty() {
            empty_state(ui, pal, "Belum ada playlist", "Buat playlist untuk mengelompokkan video.");
            return;
        }
        let pls = self.state.playlists.clone();
        for pl in &pls {
            let mut open = false;
            let mut del = false;
            Frame::none().fill(pal.surface).rounding(Rounding::same(12.0)).inner_margin(Margin::same(10.0)).show(ui, |ui| {
                ui.horizontal(|ui| {
                    let (rect, resp) = ui.allocate_exact_size(vec2(96.0, 56.0), Sense::click());
                    let p = ui.painter_at(rect);
                    p.rect_filled(rect, Rounding::same(8.0), pal.surface2);
                    if let Some(first) = pl.ids.first() {
                        if let Some(v) = self.video(first) {
                            self.draw_thumb(ui, rect, &v, 8.0);
                        }
                    }
                    p.text(rect.center(), Align2::CENTER_CENTER, icons::PLAYLIST_PLAY, FontId::new(22.0, icons::icon_family()), Color32::WHITE);
                    if resp.clicked() {
                        open = true;
                    }
                    ui.vertical(|ui| {
                        ui.label(RichText::new(trunc(&pl.name, 32)).size(14.0).family(icons::medium_family()).color(pal.ink));
                        ui.label(RichText::new(format!("{} video", pl.ids.len())).size(11.0).color(pal.muted));
                        if icon_btn(ui, icons::DELETE, 15.0, pal.muted).clicked() {
                            del = true;
                        }
                    });
                });
            });
            ui.add_space(8.0);
            if open {
                self.back_to = Some(self.screen.clone());
                self.screen = Screen::Playlist(pl.id.clone());
                self.selected.clear();
                break;
            }
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
            empty_state(ui, pal, "Belum ada video disukai", "Ketuk Suka pada video untuk menyimpannya.");
            return;
        }
        for id in &ids {
            if let Some(v) = self.video(id) {
                self.video_row(ui, &v, pal);
            }
        }
    }

    fn watch_later_tab(&mut self, ui: &mut egui::Ui, pal: &Pal) {
        let ids = self.state.watch_later.clone();
        if ids.is_empty() {
            empty_state(ui, pal, "Tonton nanti kosong", "Simpan video untuk ditonton nanti dari halaman video.");
            return;
        }
        for id in &ids {
            if let Some(v) = self.video(id) {
                self.video_row(ui, &v, pal);
            }
        }
    }

    fn video_row(&mut self, ui: &mut egui::Ui, v: &Video, pal: &Pal) {
        let mut clicked = false;
        ui.horizontal(|ui| {
            let (rect, resp) = ui.allocate_exact_size(vec2(150.0, 84.0), Sense::click());
            self.draw_thumb(ui, rect, v, 8.0);
            if resp.clicked() {
                clicked = true;
            }
            ui.vertical(|ui| {
                ui.set_max_width((ui.available_width() - 8.0).max(100.0));
                ui.add(egui::Label::new(RichText::new(trunc(&v.title, 60)).size(13.0).family(icons::medium_family()).color(pal.ink)).wrap(true));
                ui.label(RichText::new(&v.channel).size(11.0).color(pal.muted));
                ui.label(RichText::new(meta_line(v)).size(11.0).color(pal.muted));
            });
            ui.with_layout(Layout::right_to_left(Align::Min), |ui| {
                if icon_btn(ui, icons::MORE_VERT, 16.0, pal.muted).clicked() {
                    self.dialog = Some(Dialog::SaveToPlaylist(v.id.clone()));
                }
            });
        });
        if clicked {
            self.open_detail(&v.id);
        }
        ui.add_space(12.0);
    }

    // ---------- halaman pencarian ----------

    fn search_page(&mut self, ui: &mut egui::Ui, pal: &Pal) {
        ui.add_space(6.0);
        if self.search_loading {
            ui.add_space(40.0);
            ui.vertical_centered(|ui| {
                ui.spinner();
                ui.add_space(8.0);
                ui.label(RichText::new("Mencari di YouTube...").size(12.0).color(pal.muted));
            });
            return;
        }
        if let Some(err) = self.search_err.clone() {
            empty_state(ui, pal, "Pencarian gagal", &err);
            if ui.add(egui::Button::new(RichText::new("Coba lagi").size(12.0).family(icons::medium_family()).color(Color32::BLACK)).fill(pal.ink).rounding(Rounding::same(18.0))).clicked() {
                let q = self.query.clone();
                self.start_search(&q);
            }
            return;
        }
        if self.results.is_empty() {
            if self.state.recents.is_empty() {
                empty_state(ui, pal, "Telusuri YouTube", "Ketik kata kunci untuk mencari video asli dari YouTube.");
            } else {
                ui.label(RichText::new("Pencarian terakhir").size(12.0).family(icons::medium_family()).color(pal.ink));
                ui.add_space(6.0);
                let recents = self.state.recents.clone();
                for r in &recents {
                    let mut go = false;
                    let resp = Frame::none().inner_margin(Margin { left: 0.0, right: 0.0, top: 10.0, bottom: 10.0 }).show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(icons::HISTORY).family(icons::icon_family()).size(20.0).color(pal.muted));
                            ui.label(RichText::new(r).size(13.0).color(pal.ink));
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                if icon_btn(ui, icons::CLOSE, 16.0, pal.muted).clicked() {
                                    self.state.recents.retain(|x| x != r);
                                    self.mark();
                                }
                            });
                        });
                    }).response;
                    if resp.interact(Sense::click()).clicked() {
                        go = true;
                    }
                    if go {
                        self.start_search(r);
                        break;
                    }
                }
            }
            return;
        }
        ui.label(RichText::new(format!("{} hasil untuk \"{}\"", self.results.len(), trunc(&self.query, 24))).size(11.0).color(pal.muted));
        ui.add_space(8.0);
        let results = self.results.clone();
        for v in &results {
            self.video_row(ui, v, pal);
        }
        ui.add_space(16.0);
    }

    // ---------- halaman detail ----------

    fn detail_page(&mut self, ui: &mut egui::Ui, id: &str, pal: &Pal) {
        let v = match self.video(id) {
            Some(v) => v,
            None => {
                empty_state(ui, pal, "Video tidak ditemukan", "Coba kembali atau telusuri ulang.");
                return;
            }
        };
        let w = ui.available_width();
        let h = w * 9.0 / 16.0;
        let (rect, resp) = ui.allocate_exact_size(vec2(w, h), Sense::click());
        let p = ui.painter_at(rect);
        p.rect_filled(rect, Rounding::ZERO, Color32::BLACK);
        self.draw_thumb(ui, rect, &v, 0.0);
        let btn = Rect::from_center_size(rect.center(), vec2(64.0, 44.0));
        p.rect_filled(btn, Rounding::same(12.0), Color32::from_rgba_unmultiplied(255, 0, 0, 235));
        p.add(Shape::convex_polygon(
            vec![pos2(btn.center().x - 8.0, btn.center().y - 11.0), pos2(btn.center().x - 8.0, btn.center().y + 11.0), pos2(btn.center().x + 12.0, btn.center().y)],
            Color32::WHITE,
            Stroke::NONE,
        ));
        if resp.clicked() {
            self.play(ui, &v.id);
        }
        ui.add_space(12.0);
        ui.add(egui::Label::new(RichText::new(&v.title).size(17.0).family(icons::medium_family()).color(pal.ink)).wrap(true));
        ui.add_space(2.0);
        ui.horizontal(|ui| {
            let mut meta = meta_line(&v);
            if meta.is_empty() {
                meta = "YouTube".into();
            }
            ui.label(RichText::new(meta).size(12.0).color(pal.muted));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if icon_btn(ui, icons::MORE_HORIZ, 18.0, pal.ink).clicked() {
                    self.dialog = Some(Dialog::SaveToPlaylist(v.id.clone()));
                }
            });
        });
        ui.add_space(10.0);
        egui::ScrollArea::horizontal().id_source("actions").show(ui, |ui| {
            ui.horizontal(|ui| {
                let liked = self.state.likes.get(&v.id).copied().unwrap_or(false);
                if action_pill(ui, icons::THUMB_UP, if liked { "Disukai" } else { "Suka" }, liked, pal).clicked() {
                    self.toggle_like(&v.id);
                }
                if action_pill(ui, icons::SHARE, "Bagikan", false, pal).clicked() {
                    let url = format!("https://www.youtube.com/watch?v={}", v.id);
                    ui.ctx().output_mut(|o| o.copied_text = url);
                    self.toast("Link disalin");
                }
                let wl = self.state.watch_later.contains(&v.id);
                if action_pill(ui, icons::WATCH_LATER, if wl { "Tersimpan" } else { "Tonton nanti" }, wl, pal).clicked() {
                    self.toggle_watch_later(&v.id);
                }
                if action_pill(ui, icons::PLAYLIST_ADD, "Simpan", false, pal).clicked() {
                    self.dialog = Some(Dialog::SaveToPlaylist(v.id.clone()));
                }
            });
        });
        ui.add_space(12.0);
        ui.horizontal(|ui| {
            let (ar, _) = ui.allocate_exact_size(vec2(40.0, 40.0), Sense::hover());
            self.draw_avatar(ui, ar, &v.channel, v.color, v.avatar.as_deref());
            ui.vertical(|ui| {
                ui.label(RichText::new(trunc(&v.channel, 28)).size(14.0).family(icons::medium_family()).color(pal.ink));
                ui.label(RichText::new("Channel YouTube").size(11.0).color(pal.muted));
            });
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let subbed = self.state.subs.get(&v.channel_id).copied().unwrap_or(false);
                let label = if subbed { "Tersubscribe" } else { "Berlangganan" };
                let fill = if subbed { pal.surface2 } else { RED };
                let color = if subbed { pal.muted } else { Color32::WHITE };
                if ui.add(egui::Button::new(RichText::new(label).size(12.0).family(icons::medium_family()).color(color)).fill(fill).rounding(Rounding::same(18.0)).min_size(vec2(0.0, 34.0))).clicked() {
                    self.toggle_sub(&v.channel_id);
                }
            });
        });
        ui.add_space(12.0);
        Frame::none().fill(pal.surface).rounding(Rounding::same(12.0)).inner_margin(Margin::same(12.0)).show(ui, |ui| {
            ui.label(RichText::new(if v.meta.is_some() { meta_line(&v) } else { format!("{} x ditonton • {}", fmt_views(v.views), v.age) }).size(12.0).family(icons::medium_family()).color(pal.ink));
            ui.add_space(4.0);
            ui.label(RichText::new("Diputar lewat Brave dengan Shields aktif: iklan dan pelacak diblokir sebelum halaman dimuat.").size(12.0).color(pal.ink));
            if v.duration > 0 {
                ui.label(RichText::new(format!("Durasi {}", fmt_dur(v.duration))).size(11.0).color(pal.muted));
            }
        });
        if self.state.settings.sponsor {
            if self.pending.contains(&format!("s:{}", v.id)) {
                ui.add_space(8.0);
                ui.label(RichText::new("Memuat info SponsorBlock...").size(11.0).color(pal.muted));
            } else if let Some(segs) = self.segments.get(&v.id) {
                ui.add_space(8.0);
                if segs.is_empty() {
                    ui.label(RichText::new("SponsorBlock: tidak ada segmen sponsor.").size(11.0).color(pal.muted));
                } else {
                    Frame::none().fill(pal.surface).rounding(Rounding::same(12.0)).inner_margin(Margin::same(12.0)).show(ui, |ui| {
                        ui.label(RichText::new(format!("SponsorBlock • {} segmen", segs.len())).size(12.0).family(icons::medium_family()).color(pal.ink));
                        for s in segs {
                            ui.label(RichText::new(format!("{}  {} – {}", s.category, fmt_dur(s.start as u32), fmt_dur(s.end as u32))).size(11.0).color(pal.muted));
                        }
                    });
                }
            }
        }
        ui.add_space(16.0);
        ui.label(RichText::new("Komentar").size(15.0).family(icons::medium_family()).color(pal.ink));
        ui.add_space(4.0);
        ui.label(RichText::new("Komentar tidak tersedia di aplikasi ini. Buka video di Brave untuk membaca dan membalas komentar.").size(12.0).color(pal.muted));
        ui.add_space(18.0);
        ui.label(RichText::new("Video terkait").size(15.0).family(icons::medium_family()).color(pal.ink));
        ui.add_space(6.0);
        let related = self.related(&v);
        for x in &related {
            self.video_row(ui, x, pal);
        }
        ui.add_space(20.0);
    }

    // ---------- playlist ----------

    fn playlist_page(&mut self, ui: &mut egui::Ui, pid: &str, pal: &Pal) {
        let pl = match self.state.playlists.iter().find(|p| p.id == pid).cloned() {
            Some(p) => p,
            None => {
                self.screen = Screen::Tabs;
                return;
            }
        };
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            if icon_btn(ui, icons::PLAYLIST_PLAY, 20.0, pal.ink).clicked() {
                if let Some(first) = pl.ids.first().cloned() {
                    self.play(ui, &first);
                }
            }
            ui.label(RichText::new(format!("{} video", pl.ids.len())).size(11.0).color(pal.muted));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if icon_btn(ui, icons::EDIT, 18.0, pal.ink).clicked() {
                    self.rename_name = pl.name.clone();
                    self.dialog = Some(Dialog::RenamePlaylist(pl.id.clone()));
                }
                if icon_btn(ui, icons::DELETE, 18.0, pal.ink).clicked() {
                    self.state.playlists.retain(|p| p.id != pid);
                    self.screen = Screen::Tabs;
                    self.mark();
                    self.toast_undo("Playlist dihapus", UndoAction::RestorePlaylist(pl.clone()));
                }
            });
        });
        ui.add_space(6.0);
        ui.add(egui::TextEdit::singleline(&mut self.lib_search).hint_text("Cari di playlist").desired_width(ui.available_width()).margin(Margin::symmetric(12.0, 7.0)));
        ui.add_space(8.0);
        let q = self.lib_search.trim().to_lowercase();
        let mut ids: Vec<String> = pl
            .ids
            .iter()
            .filter(|id| {
                if q.is_empty() {
                    return true;
                }
                match self.video(id) {
                    Some(v) => v.title.to_lowercase().contains(&q) || v.channel.to_lowercase().contains(&q),
                    None => false,
                }
            })
            .cloned()
            .collect();
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
            empty_state(ui, pal, "Playlist kosong", "Tambahkan video lewat tombol Simpan di halaman video.");
            return;
        }
        let reorder = self.lib_sort == "Urutan";
        for id in &ids {
            let v = match self.video(id) {
                Some(v) => v,
                None => continue,
            };
            let mut clicked = false;
            let mut act: Option<&str> = None;
            ui.horizontal(|ui| {
                let mut checked = self.selected.contains(id);
                if ui.checkbox(&mut checked, "").changed() {
                    if checked {
                        self.selected.insert(id.clone());
                    } else {
                        self.selected.remove(id);
                    }
                }
                let (rect, resp) = ui.allocate_exact_size(vec2(140.0, 78.0), Sense::click());
                self.draw_thumb(ui, rect, &v, 8.0);
                if resp.clicked() {
                    clicked = true;
                }
                ui.vertical(|ui| {
                    ui.add(egui::Label::new(RichText::new(trunc(&v.title, 48)).size(13.0).family(icons::medium_family()).color(pal.ink)).wrap(true));
                    ui.label(RichText::new(&v.channel).size(11.0).color(pal.muted));
                    ui.horizontal(|ui| {
                        if icon_btn(ui, icons::PLAY, 15.0, pal.ink).clicked() {
                            act = Some("Putar");
                        }
                        if reorder {
                            if icon_btn(ui, icons::EXPAND_MORE, 15.0, pal.muted).clicked() {
                                act = Some("Bawah");
                            }
                        }
                        if icon_btn(ui, icons::DELETE, 15.0, pal.muted).clicked() {
                            act = Some("Hapus");
                        }
                    });
                });
            });
            ui.add_space(10.0);
            if clicked {
                self.open_detail(id);
                break;
            }
            match act {
                Some("Putar") => {
                    self.play(ui, id);
                    break;
                }
                Some("Hapus") => {
                    let old = pl.clone();
                    if let Some(p) = self.playlist_mut(pid) {
                        p.ids.retain(|x| x != id);
                    }
                    self.mark();
                    self.toast_undo("Video dihapus", UndoAction::RestorePlaylist(old));
                    break;
                }
                Some("Bawah") => {
                    if let Some(p) = self.playlist_mut(pid) {
                        if let Some(pos) = p.ids.iter().position(|x| x == id) {
                            if pos + 1 < p.ids.len() {
                                p.ids.swap(pos, pos + 1);
                                self.mark();
                            }
                        }
                    }
                    break;
                }
                _ => {}
            }
        }
        if !self.selected.is_empty() {
            ui.add_space(8.0);
            if ui.add(egui::Button::new(RichText::new(format!("Hapus {} video terpilih", self.selected.len())).size(12.0).color(Color32::WHITE)).fill(RED).rounding(Rounding::same(18.0))).clicked() {
                let old = pl.clone();
                let selected = self.selected.clone();
                if let Some(p) = self.playlist_mut(pid) {
                    p.ids.retain(|id| !selected.contains(id));
                }
                self.selected.clear();
                self.mark();
                self.toast_undo("Video dihapus dari playlist", UndoAction::RestorePlaylist(old));
            }
        }
    }

    // ---------- setelan & statistik ----------

    fn settings_page(&mut self, ui: &mut egui::Ui, pal: &Pal) {
        ui.add_space(8.0);
        ui.label(RichText::new("Pemutaran").size(11.0).family(icons::medium_family()).color(RED));
        ui.add_space(4.0);
        let mut changed = false;
        Self::toggle_row(ui, pal, "Mode tanpa iklan", "Buka video di Brave yang memblokir iklan.", &mut self.state.settings.block_ads, &mut changed);
        Self::toggle_row(ui, pal, "Buka otomatis", "Jika mati, link hanya disalin ke clipboard.", &mut self.state.settings.background, &mut changed);
        Self::toggle_row(ui, pal, "Info SponsorBlock", "Tampilkan segmen sponsor dari komunitas.", &mut self.state.settings.sponsor, &mut changed);
        Self::toggle_row(ui, pal, "Hemat data", "Tampilkan daftar ringkas tanpa thumbnail besar.", &mut self.state.settings.data_saver, &mut changed);
        if changed {
            self.mark();
        }
        ui.add_space(12.0);
        ui.label(RichText::new("Browser").size(11.0).family(icons::medium_family()).color(RED));
        ui.add_space(6.0);
        egui::ScrollArea::horizontal().id_source("browsers").show(ui, |ui| {
            ui.horizontal(|ui| {
                for (val, label) in [("auto", "Otomatis"), ("com.brave.browser", "Brave"), ("com.brave.browser_beta", "Brave Beta"), ("com.brave.browser_nightly", "Brave Nightly"), ("default", "Browser default")] {
                    if chip(ui, label, self.state.settings.brave_package == val, pal).clicked() {
                        self.state.settings.brave_package = val.to_string();
                        self.mark();
                    }
                }
            });
        });
        ui.add_space(14.0);
        ui.label(RichText::new("Tampilan").size(11.0).family(icons::medium_family()).color(RED));
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            for (val, label) in [("dark", "Gelap"), ("light", "Terang")] {
                if chip(ui, label, self.state.settings.theme == val, pal).clicked() {
                    self.state.settings.theme = val.to_string();
                    self.mark();
                }
            }
        });
        ui.add_space(16.0);
        ui.label(RichText::new("Data").size(11.0).family(icons::medium_family()).color(RED));
        ui.add_space(6.0);
        if ui.add(egui::Button::new(RichText::new("Reset semua data").size(13.0).color(RED)).fill(pal.surface).rounding(Rounding::same(10.0)).min_size(vec2(ui.available_width(), 44.0))).clicked() {
            self.dialog = Some(Dialog::ConfirmClear);
        }
        ui.add_space(20.0);
        ui.label(RichText::new("NoIklan v1.0.5 • Rust + egui").size(11.0).color(pal.muted));
        ui.label(RichText::new("Bukan aplikasi resmi YouTube. Video diputar di Brave.").size(11.0).color(pal.muted));
        ui.add_space(20.0);
    }

    fn toggle_row(ui: &mut egui::Ui, pal: &Pal, title: &str, desc: &str, value: &mut bool, changed: &mut bool) {
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.label(RichText::new(title).size(14.0).color(pal.ink));
                ui.label(RichText::new(desc).size(11.0).color(pal.muted));
            });
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if toggle(ui, value, pal) {
                    *changed = true;
                }
            });
        });
        ui.add_space(10.0);
    }

    fn stats_page(&mut self, ui: &mut egui::Ui, pal: &Pal) {
        let s = self.state.stats.clone();
        ui.add_space(10.0);
        let cards: [(&str, String, Color32); 3] = [
            ("Video dibuka", s.opened.to_string(), pal.ink),
            ("Iklan diblokir", s.blocked.to_string(), Color32::from_rgb(34, 229, 132)),
            ("Hari ini", s.today.to_string(), pal.ink),
        ];
        ui.horizontal(|ui| {
            let w = (ui.available_width() - 16.0) / 3.0;
            for (label, value, color) in cards {
                Frame::none().fill(pal.surface).rounding(Rounding::same(12.0)).inner_margin(Margin::same(10.0)).show(ui, |ui| {
                    ui.set_min_size(vec2(w - 20.0, 58.0));
                    ui.label(RichText::new(value).size(20.0).family(icons::medium_family()).color(color));
                    ui.label(RichText::new(label).size(10.0).color(pal.muted));
                });
            }
        });
        ui.add_space(12.0);
        Frame::none().fill(pal.surface).rounding(Rounding::same(12.0)).inner_margin(Margin::same(12.0)).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new("Waktu dihemat").size(12.0).color(pal.muted));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.label(RichText::new(format!("{} menit", s.time_saved_min)).size(14.0).family(icons::medium_family()).color(pal.ink));
                });
            });
            ui.horizontal(|ui| {
                ui.label(RichText::new("Data dihemat").size(12.0).color(pal.muted));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.label(RichText::new(format!("{} MB", s.data_saved_mb)).size(14.0).family(icons::medium_family()).color(pal.ink));
                });
            });
        });
        ui.add_space(12.0);
        ui.label(RichText::new("Iklan diblokir per kategori").size(14.0).family(icons::medium_family()).color(pal.ink));
        ui.add_space(6.0);
        let mut cats: Vec<(String, u64)> = s.per_category.iter().map(|(k, v)| (k.clone(), *v)).collect();
        cats.sort_by(|a, b| b.1.cmp(&a.1));
        if cats.is_empty() {
            empty_state(ui, pal, "Belum ada data", "Putar video pertamamu lewat Brave.");
        } else {
            let w = ui.available_width();
            let h = 150.0;
            let (rect, _) = ui.allocate_exact_size(vec2(w, h), Sense::hover());
            let p = ui.painter_at(rect);
            let max = cats.iter().map(|(_, v)| *v).max().unwrap_or(1).max(1);
            let n = cats.len().min(7);
            let bw = (w - 20.0) / n as f32;
            let hues = [RED, pal.blue, Color32::from_rgb(124, 92, 255), Color32::from_rgb(255, 176, 32), Color32::from_rgb(34, 229, 132)];
            for (i, (cat, val)) in cats.iter().take(n).enumerate() {
                let bh = (*val as f32 / max as f32) * (h - 40.0);
                let x = rect.left() + 10.0 + i as f32 * bw;
                let bar = Rect::from_min_max(pos2(x + 4.0, rect.bottom() - 20.0 - bh), pos2(x + bw - 4.0, rect.bottom() - 20.0));
                p.rect_filled(bar, Rounding::same(4.0), hues[i % hues.len()]);
                p.text(pos2(bar.center().x, bar.top() - 8.0), Align2::CENTER_CENTER, val.to_string(), FontId::proportional(10.0), pal.ink);
                p.text(pos2(bar.center().x, rect.bottom() - 8.0), Align2::CENTER_CENTER, trunc(cat, 8), FontId::proportional(9.0), pal.muted);
            }
        }
        ui.add_space(10.0);
        ui.label(RichText::new("Estimasi berdasarkan video yang dibuka lewat Brave Shields.").size(10.0).color(pal.muted));
    }

    // ---------- dialog ----------

    fn dialogs(&mut self, ctx: &egui::Context, pal: &Pal) {
        let dlg = match self.dialog.clone() {
            Some(d) => d,
            None => return,
        };
        let mut close = false;
        let frame = Frame::none().fill(pal.surface).rounding(Rounding::same(16.0)).stroke(Stroke::new(1.0_f32, pal.line)).inner_margin(Margin::same(18.0));
        let win = || {
            egui::Window::new("")
                .title_bar(false)
                .collapsible(false)
                .resizable(false)
                .anchor(Align2::CENTER_CENTER, vec2(0.0, 0.0))
                .frame(frame)
        };
        match dlg {
            Dialog::NewPlaylist => {
                win().show(ctx, |ui| {
                    ui.set_width(300.0);
                    ui.label(RichText::new("Playlist baru").size(16.0).family(icons::medium_family()).color(pal.ink));
                    ui.add_space(8.0);
                    ui.add(egui::TextEdit::singleline(&mut self.new_playlist_name).hint_text("Nama playlist").desired_width(ui.available_width()).margin(Margin::symmetric(10.0, 7.0)));
                    ui.add_space(10.0);
                    ui.horizontal(|ui| {
                        if ui.add(egui::Button::new(RichText::new("Buat").size(13.0).color(Color32::WHITE)).fill(RED).rounding(Rounding::same(18.0))).clicked() {
                            let name = self.new_playlist_name.trim().to_string();
                            if !name.is_empty() {
                                self.create_playlist(&name);
                                self.new_playlist_name.clear();
                                close = true;
                            }
                        }
                        if ui.add(egui::Button::new(RichText::new("Batal").size(13.0).color(pal.ink)).fill(pal.surface2).rounding(Rounding::same(18.0))).clicked() {
                            close = true;
                        }
                    });
                });
            }
            Dialog::RenamePlaylist(pid) => {
                win().show(ctx, |ui| {
                    ui.set_width(300.0);
                    ui.label(RichText::new("Ganti nama playlist").size(16.0).family(icons::medium_family()).color(pal.ink));
                    ui.add_space(8.0);
                    ui.add(egui::TextEdit::singleline(&mut self.rename_name).desired_width(ui.available_width()).margin(Margin::symmetric(10.0, 7.0)));
                    ui.add_space(10.0);
                    ui.horizontal(|ui| {
                        if ui.add(egui::Button::new(RichText::new("Simpan").size(13.0).color(Color32::WHITE)).fill(RED).rounding(Rounding::same(18.0))).clicked() {
                            let name = self.rename_name.trim().to_string();
                            if !name.is_empty() {
                                if let Some(p) = self.playlist_mut(&pid) {
                                    p.name = name;
                                }
                                self.mark();
                                close = true;
                            }
                        }
                        if ui.add(egui::Button::new(RichText::new("Batal").size(13.0).color(pal.ink)).fill(pal.surface2).rounding(Rounding::same(18.0))).clicked() {
                            close = true;
                        }
                    });
                });
            }
            Dialog::AddLink => {
                win().show(ctx, |ui| {
                    ui.set_width(310.0);
                    ui.label(RichText::new("Tambah dari link").size(16.0).family(icons::medium_family()).color(pal.ink));
                    ui.add_space(4.0);
                    ui.label(RichText::new("Tempel link watch, youtu.be, atau shorts.").size(11.0).color(pal.muted));
                    ui.add_space(8.0);
                    ui.add(egui::TextEdit::singleline(&mut self.link_input).hint_text("https://youtu.be/...").desired_width(ui.available_width()).margin(Margin::symmetric(10.0, 7.0)));
                    ui.add_space(10.0);
                    ui.horizontal(|ui| {
                        if ui.add(egui::Button::new(RichText::new("Tambah").size(13.0).color(Color32::WHITE)).fill(RED).rounding(Rounding::same(18.0))).clicked() {
                            let raw = self.link_input.trim().to_string();
                            match parse_yt_id(&raw) {
                                Some(id) => {
                                    if self.video(&id).is_some() {
                                        self.open_detail(&id);
                                        close = true;
                                    } else {
                                        let key = format!("o:{}", id);
                                        if !self.pending.contains(&key) {
                                            self.pending.insert(key);
                                            let tx = self.tx.clone();
                                            let id2 = id.clone();
                                            std::thread::spawn(move || {
                                                let r = youtube::fetch_oembed(&id2);
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
                        if ui.add(egui::Button::new(RichText::new("Batal").size(13.0).color(pal.ink)).fill(pal.surface2).rounding(Rounding::same(18.0))).clicked() {
                            close = true;
                        }
                    });
                });
            }
            Dialog::ConfirmClear => {
                win().show(ctx, |ui| {
                    ui.set_width(300.0);
                    ui.label(RichText::new("Reset data aplikasi?").size(16.0).family(icons::medium_family()).color(pal.ink));
                    ui.add_space(4.0);
                    ui.label(RichText::new("Playlist, riwayat, suka, dan langganan akan dihapus.").size(12.0).color(pal.muted));
                    ui.add_space(10.0);
                    ui.horizontal(|ui| {
                        if ui.add(egui::Button::new(RichText::new("Hapus semua").size(13.0).color(Color32::WHITE)).fill(RED).rounding(Rounding::same(18.0))).clicked() {
                            let old = self.state.clone();
                            self.state = State::default();
                            self.mark();
                            self.toast_undo("Data direset", UndoAction::RestoreState(Box::new(old)));
                            close = true;
                        }
                        if ui.add(egui::Button::new(RichText::new("Batal").size(13.0).color(pal.ink)).fill(pal.surface2).rounding(Rounding::same(18.0))).clicked() {
                            close = true;
                        }
                    });
                });
            }
            Dialog::SaveToPlaylist(vid) => {
                win().show(ctx, |ui| {
                    ui.set_width(310.0);
                    ui.label(RichText::new("Simpan ke playlist").size(16.0).family(icons::medium_family()).color(pal.ink));
                    ui.add_space(8.0);
                    let pls = self.state.playlists.clone();
                    if pls.is_empty() {
                        ui.label(RichText::new("Belum ada playlist.").size(12.0).color(pal.muted));
                    }
                    for pl in pls {
                        let already = pl.ids.contains(&vid);
                        let label = if already { format!("{} (tersimpan)", pl.name) } else { pl.name.clone() };
                        if ui.add(egui::Button::new(RichText::new(label).size(13.0).color(pal.ink)).fill(pal.surface2).rounding(Rounding::same(10.0)).min_size(vec2(ui.available_width(), 38.0))).clicked() {
                            if !already {
                                if let Some(p) = self.playlist_mut(&pl.id) {
                                    p.ids.push(vid.clone());
                                }
                                self.mark();
                                self.toast(format!("Disimpan ke {}", pl.name));
                            }
                            close = true;
                        }
                        ui.add_space(4.0);
                    }
                    ui.add_space(8.0);
                    ui.add(egui::TextEdit::singleline(&mut self.new_playlist_name).hint_text("Playlist baru").desired_width(ui.available_width()).margin(Margin::symmetric(10.0, 7.0)));
                    if ui.add(egui::Button::new(RichText::new("Buat & simpan").size(13.0).color(Color32::WHITE)).fill(RED).rounding(Rounding::same(18.0))).clicked() {
                        let name = self.new_playlist_name.trim().to_string();
                        if !name.is_empty() {
                            let id = self.create_playlist(&name);
                            if let Some(p) = self.playlist_mut(&id) {
                                p.ids.push(vid.clone());
                            }
                            self.new_playlist_name.clear();
                            self.mark();
                            close = true;
                        }
                    }
                });
            }
            Dialog::Stats => {
                self.dialog = None;
                self.screen = Screen::Stats;
            }
            Dialog::Settings => {
                self.dialog = None;
                self.screen = Screen::Settings;
            }
        }
        if close {
            self.dialog = None;
        }
    }

    fn snack_area(&mut self, ctx: &egui::Context, pal: &Pal) {
        let s = match self.snack.clone() {
            Some(s) => s,
            None => return,
        };
        let pos = ctx.screen_rect().center_bottom() + vec2(0.0, -84.0);
        egui::Area::new(egui::Id::new("snack")).fixed_pos(pos).order(egui::Order::Foreground).show(ctx, |ui| {
            Frame::none().fill(pal.ink).rounding(Rounding::same(6.0)).inner_margin(Margin::symmetric(14.0, 10.0)).show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(&s.msg).size(12.0).color(pal.bg));
                    if let Some(u) = s.undo.clone() {
                        if ui.add(egui::Button::new(RichText::new("Urungkan").size(12.0).family(icons::medium_family()).color(pal.blue)).frame(false)).clicked() {
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
        let expired = self.snack.as_ref().map(|s| Instant::now() > s.until).unwrap_or(false);
        if expired {
            self.snack = None;
        }
        self.apply_style(ctx);
        let pal = self.pal();
        if let Some((top, bottom)) = crate::system_insets() {
            let ppp = ctx.pixels_per_point().max(0.5);
            self.safe_top = (top / ppp).ceil();
            self.safe_bottom = (bottom / ppp).ceil();
        }
        let screen = self.screen.clone();
        match screen {
            Screen::Tabs => {
                self.top_home(ctx, &pal);
                self.bottom_nav(ctx, &pal);
                egui::CentralPanel::default()
                    .frame(Frame::none().fill(pal.bg).inner_margin(Margin { left: 12.0, right: 12.0, top: 0.0, bottom: 0.0 }))
                    .show(ctx, |ui| {
                        egui::ScrollArea::vertical().auto_shrink([false; 2]).show(ui, |ui| {
                            self.body(ui, &pal);
                        });
                    });
            }
            Screen::Search => {
                self.top_search(ctx, &pal);
                egui::CentralPanel::default()
                    .frame(Frame::none().fill(pal.bg).inner_margin(Margin { left: 12.0, right: 12.0, top: 0.0, bottom: 0.0 }))
                    .show(ctx, |ui| {
                        egui::ScrollArea::vertical().auto_shrink([false; 2]).show(ui, |ui| {
                            self.search_page(ui, &pal);
                        });
                    });
            }
            Screen::Detail(id) => {
                self.top_back(ctx, &pal, "Video", None);
                egui::CentralPanel::default()
                    .frame(Frame::none().fill(pal.bg).inner_margin(Margin { left: 12.0, right: 12.0, top: 0.0, bottom: 0.0 }))
                    .show(ctx, |ui| {
                        egui::ScrollArea::vertical().auto_shrink([false; 2]).show(ui, |ui| {
                            self.detail_page(ui, &id, &pal);
                        });
                    });
            }
            Screen::Playlist(pid) => {
                let name = self.state.playlists.iter().find(|p| p.id == pid).map(|p| p.name.clone()).unwrap_or_else(|| "Playlist".into());
                self.top_back(ctx, &pal, &name, None);
                egui::CentralPanel::default()
                    .frame(Frame::none().fill(pal.bg).inner_margin(Margin { left: 12.0, right: 12.0, top: 0.0, bottom: 0.0 }))
                    .show(ctx, |ui| {
                        egui::ScrollArea::vertical().auto_shrink([false; 2]).show(ui, |ui| {
                            self.playlist_page(ui, &pid, &pal);
                        });
                    });
            }
            Screen::Settings => {
                self.top_back(ctx, &pal, "Setelan", None);
                egui::CentralPanel::default()
                    .frame(Frame::none().fill(pal.bg).inner_margin(Margin { left: 14.0, right: 14.0, top: 0.0, bottom: 0.0 }))
                    .show(ctx, |ui| {
                        egui::ScrollArea::vertical().auto_shrink([false; 2]).show(ui, |ui| {
                            self.settings_page(ui, &pal);
                        });
                    });
            }
            Screen::Stats => {
                self.top_back(ctx, &pal, "Statistik", None);
                egui::CentralPanel::default()
                    .frame(Frame::none().fill(pal.bg).inner_margin(Margin { left: 14.0, right: 14.0, top: 0.0, bottom: 0.0 }))
                    .show(ctx, |ui| {
                        egui::ScrollArea::vertical().auto_shrink([false; 2]).show(ui, |ui| {
                            self.stats_page(ui, &pal);
                        });
                    });
            }
        }
        self.dialogs(ctx, &pal);
        self.snack_area(ctx, &pal);
        self.autosave();
        if !self.thumb_pending.is_empty() || !self.pending.is_empty() || self.feed_loading || self.search_loading {
            ctx.request_repaint_after(Duration::from_millis(120));
        }
    }
}

// ---------- komponen kecil ----------

fn icon_btn(ui: &mut egui::Ui, icon: &str, size: f32, color: Color32) -> Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(size + 20.0, 40.0), Sense::click());
    let color = if resp.hovered() { Color32::WHITE } else { color };
    ui.painter_at(rect).text(rect.center(), Align2::CENTER_CENTER, icon, FontId::new(size, icons::icon_family()), color);
    resp
}

fn chip(ui: &mut egui::Ui, label: &str, active: bool, pal: &Pal) -> Response {
    let bg = if active { pal.ink } else { pal.surface2 };
    let fg = if active { pal.bg } else { pal.ink };
    let inner = Frame::none().fill(bg).rounding(Rounding::same(8.0)).inner_margin(Margin::symmetric(12.0, 8.0)).show(ui, |ui| {
        ui.label(RichText::new(label).size(13.0).family(icons::medium_family()).color(fg));
    });
    inner.response.interact(Sense::click())
}

fn nav_item(ui: &mut egui::Ui, icon: &str, label: &str, selected: bool, pal: &Pal, w: f32) -> Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(w, 52.0), Sense::click());
    let color = if selected { pal.ink } else { pal.muted };
    let p = ui.painter_at(rect);
    p.text(pos2(rect.center().x, rect.top() + 15.0), Align2::CENTER_CENTER, icon, FontId::new(24.0, icons::icon_family()), color);
    p.text(pos2(rect.center().x, rect.bottom() - 9.0), Align2::CENTER_CENTER, label, FontId::new(10.0, icons::medium_family()), color);
    resp
}

fn plus_item(ui: &mut egui::Ui, pal: &Pal, w: f32) -> Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(w, 52.0), Sense::click());
    let p = ui.painter_at(rect);
    let box_rect = Rect::from_center_size(pos2(rect.center().x, rect.top() + 20.0), vec2(32.0, 24.0));
    p.rect_stroke(box_rect, Rounding::same(7.0), Stroke::new(1.6_f32, pal.ink));
    p.text(box_rect.center(), Align2::CENTER_CENTER, icons::ADD, FontId::new(20.0, icons::icon_family()), pal.ink);
    resp
}

fn action_pill(ui: &mut egui::Ui, icon: &str, label: &str, active: bool, pal: &Pal) -> Response {
    let bg = if active { pal.ink } else { pal.surface2 };
    let fg = if active { pal.bg } else { pal.ink };
    let inner = Frame::none().fill(bg).rounding(Rounding::same(18.0)).inner_margin(Margin::symmetric(12.0, 8.0)).show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            ui.label(RichText::new(icon).family(icons::icon_family()).size(16.0).color(fg));
            ui.label(RichText::new(label).size(12.0).family(icons::medium_family()).color(fg));
        });
    });
    inner.response.interact(Sense::click())
}

fn toggle(ui: &mut egui::Ui, on: &mut bool, pal: &Pal) -> bool {
    let (rect, resp) = ui.allocate_exact_size(vec2(38.0, 22.0), Sense::click());
    let changed = resp.clicked();
    if changed {
        *on = !*on;
    }
    let p = ui.painter_at(rect);
    let track = if *on { pal.ink } else { Color32::from_rgb(90, 90, 90) };
    p.rect_filled(rect, Rounding::same(11.0), track);
    let knob_x = if *on { rect.right() - 11.0 } else { rect.left() + 11.0 };
    p.circle_filled(pos2(knob_x, rect.center().y), 8.0, if *on { pal.bg } else { Color32::from_rgb(200, 200, 200) });
    changed
}

fn skeleton(ui: &mut egui::Ui, pal: &Pal) {
    let w = ui.available_width();
    let h = w * 9.0 / 16.0;
    let (rect, _) = ui.allocate_exact_size(vec2(w, h), Sense::hover());
    ui.painter_at(rect).rect_filled(rect, Rounding::same(12.0), pal.surface);
    ui.add_space(8.0);
    let (r2, _) = ui.allocate_exact_size(vec2(36.0, 36.0), Sense::hover());
    ui.painter_at(r2).circle_filled(r2.center(), 18.0, pal.surface2);
    let (r3, _) = ui.allocate_exact_size(vec2((w - 48.0).max(60.0), 14.0), Sense::hover());
    ui.painter_at(r3).rect_filled(r3, Rounding::same(4.0), pal.surface);
    ui.add_space(22.0);
}

fn empty_state(ui: &mut egui::Ui, pal: &Pal, title: &str, body: &str) {
    ui.add_space(30.0);
    ui.vertical_centered(|ui| {
        ui.label(RichText::new(title).size(16.0).family(icons::medium_family()).color(pal.ink));
        ui.add_space(4.0);
        ui.label(RichText::new(body).size(12.0).color(pal.muted));
    });
    ui.add_space(16.0);
}

fn meta_line(v: &Video) -> String {
    if let Some(m) = &v.meta {
        return m.clone();
    }
    let mut parts = Vec::new();
    if v.live {
        parts.push("Langsung".to_string());
    } else if v.views > 0 {
        parts.push(format!("{} x ditonton", fmt_views(v.views)));
    }
    if !v.age.is_empty() {
        parts.push(v.age.clone());
    }
    parts.join(" • ")
}

fn fmt_dur(secs: u32) -> String {
    if secs == 0 {
        return "--:--".into();
    }
    let h = secs / 3600;
    let m = (secs % 3600) / 60;
    let s = secs % 60;
    if h > 0 {
        format!("{}:{:02}:{:02}", h, m, s)
    } else {
        format!("{}:{:02}", m, s)
    }
}

fn fmt_views(n: u64) -> String {
    let s = if n >= 1_000_000_000 {
        format!("{:.1} M", n as f64 / 1e9)
    } else if n >= 1_000_000 {
        format!("{:.1} jt", n as f64 / 1e6)
    } else if n >= 1_000 {
        format!("{:.0} rb", n as f64 / 1e3)
    } else {
        n.to_string()
    };
    s.replace('.', ",")
}

fn trunc(s: &str, n: usize) -> String {
    if s.chars().count() > n {
        let t: String = s.chars().take(n.saturating_sub(1)).collect();
        format!("{}…", t)
    } else {
        s.to_string()
    }
}

fn initials(s: &str) -> String {
    let mut out = String::new();
    for w in s.split_whitespace().take(2) {
        if let Some(c) = w.chars().next() {
            out.extend(c.to_uppercase());
        }
    }
    if out.is_empty() {
        out.push('?');
    }
    out
}

fn ago(at: u64) -> String {
    let d = now().saturating_sub(at);
    if d < 60 {
        "baru saja".into()
    } else if d < 3600 {
        format!("{} mnt lalu", d / 60)
    } else if d < 86400 {
        format!("{} jam lalu", d / 3600)
    } else {
        format!("{} hari lalu", d / 86400)
    }
}

fn parse_yt_id(s: &str) -> Option<String> {
    let s = s.trim();
    let valid = |c: char| c.is_ascii_alphanumeric() || c == '-' || c == '_';
    if s.len() == 11 && s.chars().all(valid) {
        return Some(s.to_string());
    }
    let cand = if let Some(pos) = s.find("youtu.be/") {
        &s[pos + 9..]
    } else if let Some(pos) = s.find("v=") {
        &s[pos + 2..]
    } else if let Some(pos) = s.find("shorts/") {
        &s[pos + 7..]
    } else if let Some(pos) = s.find("embed/") {
        &s[pos + 6..]
    } else if let Some(pos) = s.find("live/") {
        &s[pos + 5..]
    } else {
        return None;
    };
    let id: String = cand.chars().take_while(|c| valid(*c)).collect();
    if id.len() == 11 {
        Some(id)
    } else {
        None
    }
}
