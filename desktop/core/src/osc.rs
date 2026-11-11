use rosc::{encoder, OscMessage, OscPacket, OscType};

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

        let many = (0..20)
            .map(|i| i.to_string())
            .collect::<Vec<_>>()
            .join("\n");
        assert_eq!(clamp_chatbox(&many).lines().count(), CHATBOX_MAX_LINES);
    }

    #[test]
    fn emoji_count_as_two_units() {
        let text = "🎵".repeat(100);
        let clamped = clamp_chatbox(&text);
        assert!(clamped.encode_utf16().count() <= CHATBOX_MAX_CHARS);
    }
}
