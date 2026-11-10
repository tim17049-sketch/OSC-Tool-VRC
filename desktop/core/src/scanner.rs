//! Furry scanner: a joke. It "measures" a random percentage and makes up a
//! verdict and a piece of "evidence". Nothing is actually detected.

use std::time::{SystemTime, UNIX_EPOCH};

/// Verdicts per range, (upper limit, German, English).
const TIERS: &[(u8, &[(&str, &str)])] = &[
    (10, &[
        ("Komplett fellfrei. Verdächtig glatt.", "Completely furless. Suspiciously smooth."),
        ("Nicht mal ein Schnurrhaar gefunden.", "Not a single whisker found."),
        ("Mensch. Langweilig, aber Mensch.", "Human. Boring, but human."),
    ]),
    (30, &[
        ("Leichter Flaum. Unter Beobachtung.", "Light fuzz. Under observation."),
        ("Hat mal ein Fursuit von Weitem gesehen.", "Once saw a fursuit from afar."),
        ("Fast sauber. Fast.", "Almost clean. Almost."),
    ]),
    (55, &[
        ("Halb Fell, halb Ausreden.", "Half fur, half excuses."),
        ("Der Schwanz wedelt schon ein bisschen.", "The tail is wagging a little already."),
        ("Klassischer Teilzeit-Furry.", "Classic part-time furry."),
    ]),
    (80, &[
        ("Eindeutig flauschig. Leugnen zwecklos.", "Clearly fluffy. Denial is pointless."),
        ("Die Ohren verraten alles.", "The ears give it all away."),
        ("Mehr Fell als Mensch.", "More fur than human."),
    ]),
    (99, &[
        ("Maximaler Flausch erreicht.", "Maximum fluff reached."),
        ("Der Scanner hat Haare im Lüfter.", "The scanner has fur in its fan now."),
        ("Bitte vor dem Betreten abbürsten.", "Please brush off before entering."),
    ]),
    (100, &[
        ("100 %. Der Scanner verneigt sich.", "100 %. The scanner bows."),
        ("Legendärer Ober-Furry entdeckt!", "Legendary apex furry detected!"),
    ]),
];

const EVIDENCE: &[(&str, &str)] = &[
    ("Beweis: ein verdächtiges „OwO“ im Chatverlauf", "Evidence: a suspicious \"OwO\" in the chat log"),
    ("Beweis: Pfotenabdrücke auf der Tastatur", "Evidence: paw prints on the keyboard"),
    ("Beweis: 47 Tabs mit Fursuit-Preisen", "Evidence: 47 tabs of fursuit prices"),
    ("Beweis: hat beim Scan gewedelt", "Evidence: wagged during the scan"),
    ("Beweis: Fell im Headset-Polster", "Evidence: fur in the headset padding"),
    ("Beweis: sagt „rawr“ beim Niesen", "Evidence: says \"rawr\" when sneezing"),
    ("Beweis: Avatar hat mehr Ohren als nötig", "Evidence: avatar has more ears than needed"),
    ("Beweis: keiner. Der Scanner hat einfach ein Gefühl.", "Evidence: none. The scanner just has a feeling."),
];

const SCANNING: &[(&str, &str)] = &[
    ("Messe Flauschfaktor …", "Measuring fluff factor …"),
    ("Suche nach Fell …", "Looking for fur …"),
    ("Zähle Schwänze …", "Counting tails …"),
    ("Prüfe UwU-Werte …", "Checking UwU levels …"),
    ("Kalibriere Ohrensensor …", "Calibrating ear sensor …"),
];

/// Small xorshift generator; a joke does not need a crypto RNG.
pub struct Rng(u64);

impl Rng {
    pub fn from_time() -> Self {
        let seed = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0x9e37_79b9_7f4a_7c15);
        Self(seed | 1)
    }

    pub fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }

    pub fn below(&mut self, n: usize) -> usize {
        (self.next() % n.max(1) as u64) as usize
    }
}

fn pick<'a>(rng: &mut Rng, list: &'a [(&'a str, &'a str)], en: bool) -> &'a str {
    let (de, english) = list[rng.below(list.len())];
    if en {
        english
    } else {
        de
    }
}

pub struct Verdict {
    pub percent: u8,
    pub verdict: String,
    pub evidence: String,
}

pub fn roll(rng: &mut Rng, en: bool) -> Verdict {
    let percent = rng.below(101) as u8;
    let (_, tier) = TIERS
        .iter()
        .find(|(limit, _)| percent <= *limit)
        .expect("tiers cover 0..=100");
    Verdict {
        percent,
        verdict: pick(rng, tier, en).to_string(),
        evidence: pick(rng, EVIDENCE, en).to_string(),
    }
}

pub fn scanning_text(rng: &mut Rng, name: &str, en: bool) -> String {
    let head = if en { "Scanning" } else { "Scanne" };
    format!("🔍 {head} {name} …\n{}", pick(rng, SCANNING, en))
}

pub fn result_text(name: &str, v: &Verdict) -> String {
    format!("🐾 {name}: {} % Furry\n{}\n{}", v.percent, v.verdict, v.evidence)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::osc::{clamp_chatbox, CHATBOX_MAX_CHARS};

    #[test]
    fn every_result_fits_the_chatbox() {
        let mut rng = Rng(12345);
        for _ in 0..2000 {
            for en in [false, true] {
                let v = roll(&mut rng, en);
                assert!(v.percent <= 100);
                let text = result_text("Somebody With A Long Name", &v);
                assert_eq!(clamp_chatbox(&text), text, "too long: {text}");
                assert!(text.encode_utf16().count() <= CHATBOX_MAX_CHARS);
            }
        }
    }
}
