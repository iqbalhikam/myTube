use crate::model::Segment;
use serde_json::Value;
use std::time::Duration;

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
