use crate::model::{Segment, Video};
use serde_json::Value;
use std::time::Duration;

pub fn fetch_oembed(id: &str) -> Result<Video, String> {
    let url = format!("https://www.youtube.com/oembed?url=https%3A%2F%2Fwww.youtube.com%2Fwatch%3Fv%3D{}&format=json", id);
    let resp = ureq::get(&url).timeout(Duration::from_secs(8)).call().map_err(|e| e.to_string())?;
    let v: Value = resp.into_json().map_err(|e| e.to_string())?;
    let title = v["title"].as_str().unwrap_or("Video YouTube").to_string();
    let channel = v["author_name"].as_str().unwrap_or("YouTube").to_string();
    let mut h = 0u32;
    for b in id.bytes() {
        h = h.wrapping_mul(31).wrapping_add(b as u32);
    }
    let palette = [[255, 45, 77], [46, 155, 255], [124, 92, 255], [255, 176, 32], [34, 229, 132], [25, 211, 218]];
    let color = palette[(h % 6) as usize];
    Ok(Video { id: id.into(), title, channel_id: channel.to_lowercase().replace(' ', "-"), channel, category: "Lainnya".into(), duration: 0, views: 0, age: "baru".into(), color, custom: true })
}

pub fn fetch_segments(id: &str) -> Result<Vec<Segment>, String> {
    let url = format!("https://sponsor.ajay.app/api/skipSegments?videoID={}&category=sponsor&category=selfpromo&category=interaction&category=intro&category=outro", id);
    let resp = match ureq::get(&url).timeout(Duration::from_secs(6)).call() {
        Ok(r) => r,
        Err(ureq::Error::Status(404, _)) => return Ok(Vec::new()),
        Err(e) => return Err(e.to_string()),
    };
    let v: Value = resp.into_json().map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    if let Some(arr) = v.as_array() {
        for item in arr {
            let category = item["category"].as_str().unwrap_or("sponsor").to_string();
            if let Some(seg) = item["segment"].as_array() {
                if seg.len() >= 2 {
                    let start = seg[0].as_f64().unwrap_or(0.0);
                    let end = seg[1].as_f64().unwrap_or(0.0);
                    if end > start {
                        out.push(Segment { category, start, end });
                    }
                }
            }
        }
    }
    Ok(out)
}
