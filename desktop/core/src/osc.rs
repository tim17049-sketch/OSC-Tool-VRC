use rosc::{decoder, encoder, OscMessage, OscPacket, OscType};

/// VRChat shows at most this many characters in the chatbox.
pub const CHATBOX_MAX_CHARS: usize = 144;
/// ... and at most this many lines.
pub const CHATBOX_MAX_LINES: usize = 9;

pub fn encode(addr: &str, args: Vec<OscType>) -> Vec<u8> {
    let packet = OscPacket::Message(OscMessage {
        addr: addr.to_string(),
        args,
    });
    // Encoding a plain message cannot fail.
    encoder::encode(&packet).unwrap_or_default()
}

/// `/chatbox/input <text> <send immediately> <notification sound>`
pub fn chatbox_input(text: &str, notify: bool) -> Vec<u8> {
    encode(
        "/chatbox/input",
        vec![
            OscType::String(text.to_string()),
            OscType::Bool(true),
            OscType::Bool(notify),
        ],
    )
}

/// `/chatbox/typing <bool>` toggles the typing indicator.
pub fn chatbox_typing(typing: bool) -> Vec<u8> {
    encode("/chatbox/typing", vec![OscType::Bool(typing)])
}

pub fn avatar_param(name: &str, value: OscType) -> Vec<u8> {
    encode(&format!("/avatar/parameters/{name}"), vec![value])
}

/// Prefix of every avatar parameter address.
pub const PARAM_PREFIX: &str = "/avatar/parameters/";

/// All messages in a UDP packet, bundles unpacked.
pub fn decode(buf: &[u8]) -> Vec<OscMessage> {
    fn walk(packet: OscPacket, out: &mut Vec<OscMessage>) {
        match packet {
            OscPacket::Message(m) => out.push(m),
            OscPacket::Bundle(b) => b.content.into_iter().for_each(|p| walk(p, out)),
        }
    }
    let mut out = Vec::new();
    if let Ok((_, packet)) = decoder::decode_udp(buf) {
        walk(packet, &mut out);
    }
    out
}

/// Readable form of OSC arguments for the monitor.
pub fn format_args(args: &[OscType]) -> String {
    args.iter()
        .map(|a| match a {
            OscType::Bool(b) => b.to_string(),
            OscType::Int(i) => i.to_string(),
            OscType::Long(i) => i.to_string(),
            OscType::Float(f) => format!("{f:.3}"),
            OscType::Double(f) => format!("{f:.3}"),
            OscType::String(s) => format!("\"{s}\""),
            OscType::Nil => "nil".into(),
            other => format!("{other:?}"),
        })
        .collect::<Vec<_>>()
        .join(", ")
}

/// Bool, int or float read as on/off, the way VRChat treats parameters.
pub fn truthy(arg: Option<&OscType>) -> Option<bool> {
    match arg? {
        OscType::Bool(b) => Some(*b),
        OscType::Int(i) => Some(*i != 0),
        OscType::Float(f) => Some(*f > 0.5),
        _ => None,
    }
}

/// Clean up a parameter name typed by the user. Accepts the full address as
/// well. `None` if it cannot be used in an OSC address.
pub fn param_name(input: &str) -> Option<String> {
    let name = input.trim();
    let name = name.strip_prefix(PARAM_PREFIX).unwrap_or(name).trim_matches('/');
    let bad = |c: char| c.is_whitespace() || c.is_control() || "#*,?[]{}".contains(c);
    (!name.is_empty() && name.len() <= 128 && !name.chars().any(bad)).then(|| name.to_string())
}

/// Cut text down to what the chatbox can display. VRChat counts UTF-16 code
/// units, so emoji weigh two.
pub fn clamp_chatbox(text: &str) -> String {
    let limited_lines: Vec<&str> = text.lines().take(CHATBOX_MAX_LINES).collect();
    let joined = limited_lines.join("\n");
    if joined.encode_utf16().count() <= CHATBOX_MAX_CHARS {
        return joined;
    }
    let mut out = String::new();
    let mut units = 0;
    for c in joined.chars() {
        // Keep one unit free for the ellipsis.
        if units + c.len_utf16() > CHATBOX_MAX_CHARS - 1 {
            break;
        }
        units += c.len_utf16();
        out.push(c);
    }
    out.push('…');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chatbox_packet_layout() {
        let buf = chatbox_input("hi", false);
        // address, padded to 16 bytes
        assert_eq!(&buf[..16], b"/chatbox/input\0\0");
        // type tags: string, true, false
        assert_eq!(&buf[16..20], b",sTF");
        assert_eq!(buf.len() % 4, 0);
    }

    #[test]
    fn clamps_length_and_lines() {
        let long = "x".repeat(300);
        let clamped = clamp_chatbox(&long);
        assert_eq!(clamped.encode_utf16().count(), CHATBOX_MAX_CHARS);
        assert!(clamped.ends_with('…'));

        let many = (0..20).map(|i| i.to_string()).collect::<Vec<_>>().join("\n");
        assert_eq!(clamp_chatbox(&many).lines().count(), CHATBOX_MAX_LINES);
    }

    #[test]
    fn decodes_what_it_encodes() {
        let messages = decode(&avatar_param("Ears", OscType::Bool(true)));
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].addr, "/avatar/parameters/Ears");
        assert_eq!(truthy(messages[0].args.first()), Some(true));
        assert!(decode(b"garbage").is_empty());
    }

    #[test]
    fn checks_param_names() {
        assert_eq!(param_name(" Ears ").as_deref(), Some("Ears"));
        assert_eq!(param_name("/avatar/parameters/Go/Hat").as_deref(), Some("Go/Hat"));
        assert_eq!(param_name("two words"), None);
        assert_eq!(param_name(""), None);
    }

    #[test]
    fn emoji_count_as_two_units() {
        let text = "🎵".repeat(100);
        let clamped = clamp_chatbox(&text);
        assert!(clamped.encode_utf16().count() <= CHATBOX_MAX_CHARS);
    }
}
