use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

pub fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs()
}

pub fn day() -> u64 {
    now() / 86_400
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Video {
    pub id: String,
    pub title: String,
    pub channel: String,
    pub channel_id: String,
    pub category: String,
    pub duration: u32,
    pub views: u64,
    pub age: String,
    pub color: [u8; 3],
    pub custom: bool,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Settings {
    pub block_ads: bool,
    pub sponsor: bool,
    pub background: bool,
    pub data_saver: bool,
    pub theme: String,
    pub brave_package: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self { block_ads: true, sponsor: true, background: true, data_saver: false, theme: "dark".into(), brave_package: "auto".into() }
    }
}

#[derive(Clone, Serialize, Deserialize, Default)]
pub struct Stats {
    pub opened: u64,
    pub blocked: u64,
    pub today: u64,
    pub time_saved_min: u64,
    pub data_saved_mb: u64,
    pub per_category: HashMap<String, u64>,
    pub last_day: u64,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct HistoryItem {
    pub id: String,
    pub at: u64,
    pub progress: f32,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Playlist {
    pub id: String,
    pub name: String,
    pub ids: Vec<String>,
    pub created: u64,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Segment {
    pub category: String,
    pub start: f64,
    pub end: f64,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct State {
    pub settings: Settings,
    pub stats: Stats,
    pub likes: HashMap<String, bool>,
    pub subs: HashMap<String, bool>,
    pub history: Vec<HistoryItem>,
    pub watch_later: Vec<String>,
    pub playlists: Vec<Playlist>,
    pub recents: Vec<String>,
    pub custom_videos: Vec<Video>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            settings: Settings::default(),
            stats: Stats::default(),
            likes: HashMap::new(),
            subs: HashMap::new(),
            history: Vec::new(),
            watch_later: Vec::new(),
            playlists: Vec::new(),
            recents: Vec::new(),
            custom_videos: Vec::new(),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Tab { Home, Search, Subs, Library }

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum LibraryTab { History, Playlists, Likes, WatchLater }

#[derive(Clone)]
pub enum Dialog {
    NewPlaylist,
    RenamePlaylist(String),
    AddLink,
    ConfirmClear,
    SaveToPlaylist(String),
    Stats,
    Settings,
}

#[derive(Clone)]
pub enum UndoAction {
    RestoreHistory(Vec<HistoryItem>),
    RestorePlaylist(Playlist),
    RestoreLike(String),
    RestoreWatchLater(String),
    RestoreState(Box<State>),
}

pub enum AsyncResult {
    OEmbed { id: String, result: Result<Video, String> },
    Sponsor { id: String, result: Result<Vec<Segment>, String> },
}
