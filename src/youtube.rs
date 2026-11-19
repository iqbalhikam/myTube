use crate::model::Video;
use serde_json::Value;
use std::collections::HashSet;
use std::io::Read;
use std::time::Duration;

const UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36";
const PALETTE: [[u8; 3]; 6] = [[255, 45, 77], [46, 155, 255], [124, 92, 255], [255, 176, 32], [34, 229, 132], [25, 211, 218]];

pub fn thumb_url(id: &str) -> String {
    format!("https://i.ytimg.com/vi/{}/mqdefault.jpg", id)
}

fn color_for(id: &str) -> [u8; 3] {
    let mut h = 0u32;
    for b in id.bytes() {
        h = h.wrapping_mul(31).wrapping_add(b as u32);
    }
    PALETTE[(h % 6) as usize]
}

fn encode(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(b as char),
            b' ' => out.push('+'),
            _ => out.push_str(&format!("%{:02X}", b)),
        }
    }
    out
}

fn fetch(url: &str) -> Result<Vec<u8>, String> {
    let resp = ureq::get(url)
        .set("User-Agent", UA)
        .set("Accept-Language", "id-ID,id;q=0.9,en;q=0.6")
        .timeout(Duration::from_secs(12))
        .call()
        .map_err(|e| match e {
            ureq::Error::Status(code, _) => format!("YouTube membalas HTTP {}", code),
            other => other.to_string(),
        })?;
    let mut buf = Vec::new();
    resp.into_reader()
        .take(8 * 1024 * 1024)
        .read_to_end(&mut buf)
        .map_err(|e| e.to_string())?;
    Ok(buf)
}

/// Ambil objek JSON setelah `ytInitialData = {` di HTML YouTube.
fn initial_data(html: &str) -> Option<Value> {
    let pos = html.find("ytInitialData")?;
    let rest = &html[pos + "ytInitialData".len()..];
    let start = rest.find('{')?;
    let bytes = rest.as_bytes();
    let mut depth = 0i32;
    let mut in_str = false;
    let mut escaped = false;
    let mut end = None;
    for (i, &b) in bytes.iter().enumerate().skip(start) {
        if in_str {
            if escaped {
                escaped = false;
            } else if b == b'\\' {
                escaped = true;
            } else if b == b'"' {
                in_str = false;
            }
            continue;
        }
        match b {
            b'"' => in_str = true,
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    end = Some(i + 1);
                    break;
                }
            }
            _ => {}
        }
    }
    serde_json::from_str(&rest[start..end?]).ok()
}

fn runs_text(v: &Value) -> Option<String> {
    if let Some(s) = v.as_str() {
        return Some(s.to_string());
    }
    if let Some(runs) = v["runs"].as_array() {
        let mut out = String::new();
        for r in runs {
            if let Some(t) = r["text"].as_str() {
                out.push_str(t);
            }
        }
        if !out.is_empty() {
            return Some(out);
        }
    }
    v["simpleText"].as_str().map(|s| s.to_string())
}

fn parse_dur(s: &str) -> u32 {
    let mut secs = 0u32;
    for part in s.trim().split(':') {
        if let Ok(n) = part.parse::<u32>() {
            secs = secs * 60 + n;
        }
    }
    secs
}

fn largest_thumb(node: &Value) -> Option<String> {
    node["thumbnails"]
        .as_array()
        .and_then(|a| a.last())
        .and_then(|t| t["url"].as_str())
        .map(|s| s.to_string())
}

fn from_renderer(vr: &Value) -> Option<Video> {
    let id = vr["videoId"].as_str()?.to_string();
    if id.len() != 11 {
        return None;
    }
    let title = runs_text(&vr["title"]).unwrap_or_else(|| "Tanpa judul".into());
    let channel = runs_text(&vr["ownerText"])
        .or_else(|| runs_text(&vr["longBylineText"]))
        .or_else(|| runs_text(&vr["shortBylineText"]))
        .unwrap_or_else(|| "YouTube".into());
    let channel_slug = channel.to_lowercase().replace(' ', "-");
    let channel_id = vr["ownerText"]["runs"][0]["navigationEndpoint"]["browseEndpoint"]["browseId"]
        .as_str()
        .unwrap_or("")
        .to_string();
    let channel_id = if channel_id.is_empty() { channel_slug } else { channel_id };
    let avatar = largest_thumb(&vr["channelThumbnailSupportedRenderers"]["channelThumbnailWithLinkRenderer"]["thumbnail"]);
    let duration = vr["lengthText"]["simpleText"].as_str().map(parse_dur).unwrap_or(0);
    let views_text = vr["viewCountText"]["simpleText"]
        .as_str()
        .or_else(|| vr["shortViewCountText"]["simpleText"].as_str())
        .unwrap_or("")
        .to_string();
    let age = vr["publishedTimeText"]["simpleText"].as_str().unwrap_or("").to_string();
    let live = vr["badges"].to_string().contains("LIVE_NOW");
    let meta = [views_text.as_str(), age.as_str()]
        .into_iter()
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(" • ");
    Some(Video {
        id: id.clone(),
        title,
        channel,
        channel_id,
        category: "YouTube".into(),
        duration,
        views: 0,
        age,
        color: color_for(&id),
        custom: false,
        meta: if meta.is_empty() { None } else { Some(meta) },
        avatar,
        live,
    })
}

fn collect(node: &Value, out: &mut Vec<Video>, seen: &mut HashSet<String>) {
    match node {
        Value::Object(map) => {
            if let Some(vr) = map.get("videoRenderer") {
                if let Some(v) = from_renderer(vr) {
                    if seen.insert(v.id.clone()) {
                        out.push(v);
                    }
                }
            }
            for (_, child) in map {
                collect(child, out, seen);
            }
        }
        Value::Array(arr) => {
            for child in arr {
                collect(child, out, seen);
            }
        }
        _ => {}
    }
}

fn page(url: &str) -> Result<Vec<Video>, String> {
    let bytes = fetch(url)?;
    let html = String::from_utf8_lossy(&bytes);
    let data = initial_data(&html).ok_or_else(|| "Gagal membaca data YouTube".to_string())?;
    let mut out = Vec::new();
    collect(&data, &mut out, &mut HashSet::new());
    Ok(out)
}

fn search_html(q: &str) -> Result<Vec<Video>, String> {
    let url = format!("https://www.youtube.com/results?search_query={}&hl=id&gl=ID&persist_gl=1", encode(q));
    page(&url)
}

pub fn search(q: &str) -> Result<Vec<Video>, String> {
    let mut v = search_html(q)?;
    if v.is_empty() {
        return Err("YouTube tidak mengembalikan hasil untuk kata kunci ini".into());
    }
    v.truncate(40);
    Ok(v)
}

pub fn trending() -> Result<Vec<Video>, String> {
    // Beranda anonim YouTube kini kosong tanpa login; susun feed dari
    // pencarian topik populer agar tetap berisi video nyata.
    const QUERIES: [&str; 3] = ["trending indonesia hari ini", "musik indonesia terbaru", "video viral indonesia"];
    let mut sets: Vec<Vec<Video>> = Vec::new();
    let mut last_err: Option<String> = None;
    std::thread::scope(|s| {
        let handles: Vec<_> = QUERIES.iter().map(|q| s.spawn(move || search_html(q))).collect();
        for h in handles {
            match h.join() {
                Ok(Ok(v)) => sets.push(v),
                Ok(Err(e)) => last_err = Some(e),
                Err(_) => last_err = Some("Gagal memuat YouTube".into()),
            }
        }
    });
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    for v in sets.into_iter().flatten() {
        if seen.insert(v.id.clone()) {
            out.push(v);
        }
    }
    if out.is_empty() {
        return Err(last_err.unwrap_or_else(|| "YouTube tidak mengembalikan hasil".into()));
    }
    out.truncate(40);
    Ok(out)
}

pub fn fetch_thumb(url: &str) -> Result<(usize, usize, Vec<u8>), String> {
    let bytes = fetch(url)?;
    let img = image::load_from_memory(&bytes).map_err(|e| e.to_string())?;
    let rgba = img.to_rgba8();
    let (w, h) = (rgba.width() as usize, rgba.height() as usize);
    if w * h > 4_000_000 {
        return Err("gambar terlalu besar".into());
    }
    Ok((w, h, rgba.into_raw()))
}

pub fn fetch_oembed(id: &str) -> Result<Video, String> {
    let url = format!("https://www.youtube.com/oembed?url=https%3A%2F%2Fwww.youtube.com%2Fwatch%3Fv%3D{}&format=json", id);
    let bytes = fetch(&url)?;
    let v: Value = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
    let title = v["title"].as_str().unwrap_or("Video YouTube").to_string();
    let channel = v["author_name"].as_str().unwrap_or("YouTube").to_string();
    Ok(Video {
        id: id.into(),
        title,
        channel_id: channel.to_lowercase().replace(' ', "-"),
        channel,
        category: "YouTube".into(),
        duration: 0,
        views: 0,
        age: "baru".into(),
        color: color_for(id),
        custom: true,
        meta: None,
        avatar: None,
        live: false,
    })
}
