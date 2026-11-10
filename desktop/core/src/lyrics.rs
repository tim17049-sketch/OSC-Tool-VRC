//! Synced lyrics from LRCLIB (https://lrclib.net), a free lyrics database
//! without accounts or API keys. Only artist, title and duration of the
//! current song are sent, and only while the template uses `{lyric}`.

use serde::Deserialize;
use std::time::Duration;

/// Lines with their start time in milliseconds, sorted.
pub type Lines = Vec<(u64, String)>;

const SEARCH_URL: &str = "https://lrclib.net/api/search";

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Hit {
    duration: Option<f64>,
    synced_lyrics: Option<String>,
    instrumental: Option<bool>,
}

/// Parse LRC text: "[01:02.34] some words". Lines without a timestamp are
/// skipped; a line with several timestamps is repeated at each of them.
pub fn parse_lrc(text: &str) -> Lines {
    let mut out = Vec::new();
    for raw in text.lines() {
        let mut rest = raw.trim();
        let mut stamps = Vec::new();
        while let Some(after) = rest.strip_prefix('[') {
            let Some(close) = after.find(']') else { break };
            match parse_stamp(&after[..close]) {
                Some(ms) => stamps.push(ms),
                None => break, // metadata tag such as [ar:Artist]
            }
            rest = after[close + 1..].trim_start();
        }
        for ms in stamps {
            out.push((ms, rest.trim().to_string()));
        }
    }
    out.sort_by_key(|(ms, _)| *ms);
    out
}

fn parse_stamp(stamp: &str) -> Option<u64> {
    let (min, sec) = stamp.split_once(':')?;
    let min: u64 = min.trim().parse().ok()?;
    let sec: f64 = sec.trim().parse().ok()?;
    if !(0.0..60.0).contains(&sec) {
        return None;
    }
    Some(min * 60_000 + (sec * 1000.0).round() as u64)
}

/// The line being sung at `pos_ms`. An empty line (instrumental part) comes
/// back as "♪"; before the first line there is nothing.
pub fn line_at(lines: &Lines, pos_ms: u64) -> Option<String> {
    let idx = lines.partition_point(|(ms, _)| *ms <= pos_ms);
    let (_, text) = lines.get(idx.checked_sub(1)?)?;
    Some(if text.is_empty() { "♪".to_string() } else { text.clone() })
}

/// "Song (feat. X) - 2011 Remaster" -> "Song"
fn simplify_title(title: &str) -> &str {
    let cut = [" - ", " (", " ["]
        .iter()
        .filter_map(|sep| title.find(sep))
        .min()
        .unwrap_or(title.len());
    title[..cut].trim()
}

/// HTTPS through the system's TLS (SChannel on Windows): no bundled crypto,
/// and the Windows certificate store is respected.
fn agent() -> Result<ureq::Agent, String> {
    let tls = native_tls::TlsConnector::new().map_err(|e| e.to_string())?;
    Ok(ureq::AgentBuilder::new().tls_connector(std::sync::Arc::new(tls)).build())
}

fn search(artist: &str, title: &str, duration_ms: Option<u64>) -> Result<Option<Lines>, String> {
    let mut request = agent()?
        .get(SEARCH_URL)
        .timeout(Duration::from_secs(8))
        .set("User-Agent", concat!("OSC-Tool-VRC/", env!("CARGO_PKG_VERSION")))
        .query("track_name", title);
    if !artist.trim().is_empty() {
        request = request.query("artist_name", artist);
    }
    let body = match request.call() {
        Ok(response) => response.into_string().map_err(|e| e.to_string())?,
        Err(ureq::Error::Status(404, _)) => return Ok(None),
        Err(e) => return Err(e.to_string()),
    };
    let hits: Vec<Hit> = serde_json::from_str(&body).map_err(|e| e.to_string())?;
    let wanted = duration_ms.map(|d| d as f64 / 1000.0);
    let best = hits
        .into_iter()
        .filter(|h| h.instrumental != Some(true))
        .filter_map(|h| Some((h.duration, h.synced_lyrics.filter(|s| !s.trim().is_empty())?)))
        // A different version of the song has different timing.
        .filter(|(d, _)| match (wanted, d) {
            (Some(w), Some(d)) => (w - d).abs() <= 4.0,
            _ => true,
        })
        .min_by(|(a, _), (b, _)| {
            let off = |d: &Option<f64>| match (wanted, d) {
                (Some(w), Some(d)) => (w - d).abs(),
                _ => 0.0,
            };
            off(a).total_cmp(&off(b))
        });
    Ok(best.map(|(_, lrc)| parse_lrc(&lrc)).filter(|l| !l.is_empty()))
}

/// Look up synced lyrics. Blocking; run it off the async runtime.
/// `Ok(None)` means LRCLIB has no synced lyrics for this song.
pub fn fetch(artist: &str, title: &str, duration_ms: Option<u64>) -> Result<Option<Lines>, String> {
    if let Some(lines) = search(artist, title, duration_ms)? {
        return Ok(Some(lines));
    }
    let simple = simplify_title(title);
    if simple != title && !simple.is_empty() {
        return search(artist, simple, duration_ms);
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_lrc() {
        let lines = parse_lrc("[ar:Someone]\n[00:01.50] Hello\n[00:03.00][00:10.00]Again\n[00:05.00]\nno stamp");
        assert_eq!(
            lines,
            vec![
                (1500, "Hello".to_string()),
                (3000, "Again".to_string()),
                (5000, String::new()),
                (10000, "Again".to_string()),
            ]
        );
    }

    #[test]
    fn finds_current_line() {
        let lines = parse_lrc("[00:01.00]One\n[00:02.00]\n[00:03.00]Three");
        assert_eq!(line_at(&lines, 500), None);
        assert_eq!(line_at(&lines, 1000).as_deref(), Some("One"));
        assert_eq!(line_at(&lines, 2500).as_deref(), Some("♪"));
        assert_eq!(line_at(&lines, 99_000).as_deref(), Some("Three"));
    }

    #[test]
    fn simplifies_titles() {
        assert_eq!(simplify_title("Song - 2011 Remaster"), "Song");
        assert_eq!(simplify_title("Song (feat. X)"), "Song");
        assert_eq!(simplify_title("Song"), "Song");
    }
}
