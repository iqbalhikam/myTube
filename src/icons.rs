use egui::{FontData, FontDefinitions, FontFamily};

pub const HOME: &str = "\u{e88a}";
pub const SEARCH: &str = "\u{e8b6}";
pub const MIC: &str = "\u{e029}";
pub const SUBS: &str = "\u{e064}";
pub const PERSON: &str = "\u{e7fd}";
pub const SHORTS: &str = "\u{e068}";
pub const PLAY: &str = "\u{e037}";
pub const PLAY_CIRCLE: &str = "\u{e039}";
pub const CAST: &str = "\u{e307}";
pub const BELL: &str = "\u{e7f4}";
pub const MORE_VERT: &str = "\u{e5d4}";
pub const MORE_HORIZ: &str = "\u{e5d3}";
pub const THUMB_UP: &str = "\u{e8dc}";
pub const THUMB_DOWN: &str = "\u{e8db}";
pub const SHARE: &str = "\u{e80d}";
pub const PLAYLIST_ADD: &str = "\u{e03b}";
pub const DOWNLOAD: &str = "\u{f090}";
pub const SAVE: &str = "\u{e867}";
pub const HISTORY: &str = "\u{e889}";
pub const WATCH_LATER: &str = "\u{e924}";
pub const FAVORITE: &str = "\u{e87d}";
pub const ARROW_BACK: &str = "\u{e5c4}";
pub const CLOSE: &str = "\u{e5cd}";
pub const ADD: &str = "\u{e145}";
pub const CHECK: &str = "\u{e5ca}";
pub const EXPAND_MORE: &str = "\u{e5cf}";
pub const CHEVRON_RIGHT: &str = "\u{e5cc}";
pub const DELETE: &str = "\u{e872}";
pub const EDIT: &str = "\u{e3c9}";
pub const LINK: &str = "\u{e157}";
pub const SETTINGS: &str = "\u{e8b8}";
pub const INSIGHTS: &str = "\u{f092}";
pub const SECURITY: &str = "\u{e32a}";
pub const SHIELD: &str = "\u{e9e0}";
pub const REFRESH: &str = "\u{e5d5}";
pub const WIFI_OFF: &str = "\u{e648}";
pub const COMMENT: &str = "\u{e0b9}";
pub const VIDEO_LIBRARY: &str = "\u{e04a}";
pub const LIBRARY_MUSIC: &str = "\u{e030}";
pub const SORT: &str = "\u{e164}";
pub const TUNE: &str = "\u{e429}";
pub const VERIFIED: &str = "\u{ef76}";
pub const LIVE_TV: &str = "\u{e639}";
pub const SPORTS: &str = "\u{ea28}";
pub const NEWSPAPER: &str = "\u{eb81}";
pub const MOVIE: &str = "\u{e02c}";
pub const PODCASTS: &str = "\u{f048}";
pub const LIGHTBULB: &str = "\u{e0f0}";
pub const MUSIC_NOTE: &str = "\u{e405}";
pub const UPLOAD: &str = "\u{f09b}";
pub const OPEN_IN_NEW: &str = "\u{e89e}";
pub const PLAYLIST_PLAY: &str = "\u{e05f}";
pub const SHUFFLE: &str = "\u{e043}";
pub const MENU: &str = "\u{e5d2}";
pub const LANGUAGE: &str = "\u{e894}";
pub const INFO: &str = "\u{e88e}";
pub const HELP: &str = "\u{e8fd}";
pub const EXPLORE: &str = "\u{e87a}";
pub const TRENDING: &str = "\u{e80e}";
pub const QUEUE_MUSIC: &str = "\u{e03d}";
pub const ACCOUNT_CIRCLE: &str = "\u{e853}";
pub const PLAYLIST_ADD_CHECK: &str = "\u{e065}";
pub const FOLDER: &str = "\u{e2c7}";

pub fn icon_family() -> FontFamily {
    FontFamily::Name("icons".into())
}

pub fn medium_family() -> FontFamily {
    FontFamily::Name("medium".into())
}

pub fn install(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();
    fonts.font_data.insert("roboto".to_owned(), FontData::from_static(include_bytes!("../assets/Roboto-Regular.ttf")));
    fonts.font_data.insert("roboto-medium".to_owned(), FontData::from_static(include_bytes!("../assets/Roboto-Medium.ttf")));
    fonts.font_data.insert("material-icons".to_owned(), FontData::from_static(include_bytes!("../assets/MaterialIcons-Regular.ttf")));
    if let Some(list) = fonts.families.get_mut(&FontFamily::Proportional) {
        list.insert(0, "roboto".to_owned());
    }
    if let Some(list) = fonts.families.get_mut(&FontFamily::Monospace) {
        list.push("roboto".to_owned());
    }
    fonts.families.insert(FontFamily::Name("medium".into()), vec!["roboto-medium".to_owned()]);
    fonts.families.insert(FontFamily::Name("icons".into()), vec!["material-icons".to_owned()]);
    ctx.set_fonts(fonts);
}
