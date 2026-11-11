use std::collections::HashMap;

/// Values a template can reference. `None` means "known placeholder, but there
/// is nothing to show right now" (no watch connected, nothing playing, ...).
pub type Values = HashMap<&'static str, Option<String>>;

/// All placeholders the engine fills in, in the order the UI lists them.
pub const PLACEHOLDERS: &[&str] = &[
    "hr", "hr_trend", "heart", "hr_min", "hr_max", "hr_avg", "song", "title", "artist", "time",
    "date", "cpu", "cpu_temp", "ram", "gpu", "gpu_temp",
];

/// Replace `{placeholders}` in `template`.
///
/// A line that uses a placeholder without a current value is dropped as a
/// whole, so "🎵 {song}" simply disappears while nothing is playing. Unknown
/// placeholders are left untouched.
pub fn render(template: &str, values: &Values) -> String {
    let mut lines = Vec::new();
    for line in template.lines() {
        if let Some(rendered) = render_line(line, values) {
            lines.push(rendered);
        }
    }
    // Trim blank lines that are left over at the edges.
    while lines.first().is_some_and(|l| l.trim().is_empty()) {
        lines.remove(0);
    }
    while lines.last().is_some_and(|l| l.trim().is_empty()) {
        lines.pop();
    }
    lines.join("\n")
}

fn render_line(line: &str, values: &Values) -> Option<String> {
    let mut out = String::with_capacity(line.len());
    let mut rest = line;
    while let Some(open) = rest.find('{') {
        out.push_str(&rest[..open]);
        let after = &rest[open + 1..];
        match after.find('}') {
            Some(close) => {
                let name = &after[..close];
                match values.get(name) {
                    Some(Some(value)) => out.push_str(value),
                    Some(None) => return None,
                    None => {
                        out.push('{');
                        out.push_str(name);
                        out.push('}');
                    }
                }
                rest = &after[close + 1..];
            }
            None => {
                out.push('{');
                rest = after;
            }
        }
    }
    out.push_str(rest);
    Some(out.trim_end().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn values() -> Values {
        let mut v = Values::new();
        v.insert("hr", Some("92".into()));
        v.insert("song", None);
        v.insert("time", Some("21:04".into()));
        v
    }

    #[test]
    fn fills_placeholders() {
        assert_eq!(render("❤ {hr} bpm", &values()), "❤ 92 bpm");
    }

    #[test]
    fn drops_lines_without_value() {
        assert_eq!(
            render("❤ {hr}\n🎵 {song}\n{time}", &values()),
            "❤ 92\n21:04"
        );
    }

    #[test]
    fn keeps_unknown_placeholders_and_stray_braces() {
        assert_eq!(render("{nope} {hr} {", &values()), "{nope} 92 {");
    }

    #[test]
    fn empty_when_nothing_available() {
        assert_eq!(render("🎵 {song}", &values()), "");
    }
}
