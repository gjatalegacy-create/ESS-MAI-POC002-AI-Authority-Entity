//! R15 transport-only codec. No verdict, trigger, capability, or persistence.
//!
//! Safe legacy representations are emitted unchanged (old receipts keep their
//! hashes). Otherwise paired xi2:/yi2: or ~np2: carry lowercase hex of a list:
//! u64 LE count, then u64 LE UTF-8 byte length and exact bytes per field.
//! No trimming, punctuation replacement, or Unicode normalization in v2.
//! Bounds and full consumption are checked before accepting untrusted input.

const MAX_BYTES: usize = crate::MAX_FIELD_BYTES;
const MAX_ITEMS: usize = crate::MAX_VECTOR_ITEMS;

fn legacy_text(text: &str, separators: &[char]) -> bool {
    !text.is_empty()
        && text.trim() == text
        && !text.chars().any(|c| c.is_control() || separators.contains(&c))
}

fn legacy_terms(terms: &[String]) -> bool {
    terms.iter().all(|s| legacy_text(s, &[',', '|']))
}

fn hex_append(out: &mut String, bytes: &[u8]) {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 15) as usize] as char);
    }
}

fn encode_list(terms: &[&str]) -> String {
    let mut out = String::new();
    hex_append(&mut out, &(terms.len() as u64).to_le_bytes());
    for text in terms {
        hex_append(&mut out, &(text.len() as u64).to_le_bytes());
        hex_append(&mut out, text.as_bytes());
    }
    out
}

fn nibble(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        _ => None,
    }
}

fn take_u64(bytes: &mut &[u8]) -> Option<usize> {
    let head: [u8; 8] = bytes.get(..8)?.try_into().ok()?;
    *bytes = bytes.get(8..)?;
    usize::try_from(u64::from_le_bytes(head)).ok()
}

fn decode_list(hex: &str) -> Option<Vec<String>> {
    match hex.len() % 2 == 0 && hex.len() / 2 <= MAX_BYTES {
        true => {}
        false => return None,
    }
    let mut binary = Vec::new();
    binary.try_reserve_exact(hex.len() / 2).ok()?;
    for pair in hex.as_bytes().chunks_exact(2) {
        binary.push((nibble(pair[0])? << 4) | nibble(pair[1])?);
    }
    let mut remaining = binary.as_slice();
    let count = take_u64(&mut remaining)?;
    match count <= MAX_ITEMS && count <= remaining.len() / 8 {
        true => {}
        false => return None,
    }
    let mut fields = Vec::new();
    fields.try_reserve_exact(count).ok()?;
    for _ in 0..count {
        let len = take_u64(&mut remaining)?;
        let text = std::str::from_utf8(remaining.get(..len)?).ok()?;
        fields.push(text.to_owned());
        remaining = remaining.get(len..)?;
    }
    match remaining.is_empty() {
        true => Some(fields),
        false => None,
    }
}

pub fn encode_split(xi: &[String], yi: &[String]) -> String {
    match legacy_terms(xi) && legacy_terms(yi) {
        true => format!("xi:{}|yi:{}", xi.join(","), yi.join(",")),
        false => format!(
            "xi2:{}|yi2:{}",
            encode_list(&xi.iter().map(String::as_str).collect::<Vec<_>>()),
            encode_list(&yi.iter().map(String::as_str).collect::<Vec<_>>()),
        ),
    }
}

fn decode_legacy_terms(text: &str) -> Option<Vec<String>> {
    match text.len() <= MAX_BYTES {
        true => {}
        false => return None,
    }
    match text.is_empty() {
        true => Some(Vec::new()),
        false => {
            match text.split(',').take(MAX_ITEMS + 1).count() <= MAX_ITEMS {
                true => {}
                false => return None,
            }
            let fields = text.split(',').map(str::to_owned).collect::<Vec<_>>();
            match fields.len() <= MAX_ITEMS && legacy_terms(&fields) {
                true => Some(fields),
                false => None,
            }
        }
    }
}

pub fn decode_split(xi: &str, yi: &str) -> Option<(Vec<String>, Vec<String>)> {
    match (xi.strip_prefix("xi2:"), yi.strip_prefix("yi2:")) {
        (Some(x), Some(y)) => {
            let pair = (decode_list(x)?, decode_list(y)?);
            // One canonical representation per decoded pair; no version mixing.
            match encode_split(&pair.0, &pair.1) == format!("{xi}|{yi}") {
                true => Some(pair),
                false => None,
            }
        }
        (None, None) => Some((
            decode_legacy_terms(xi.strip_prefix("xi:")?)?,
            decode_legacy_terms(yi.strip_prefix("yi:")?)?,
        )),
        _ => None,
    }
}

fn legacy_negative(term: &str, reason: &str) -> bool {
    legacy_text(term, &['~', '|']) && legacy_text(reason, &['~', '|'])
}

pub fn encode_negative(term: &str, space: &str, reason: &str) -> String {
    match legacy_negative(term, reason) {
        true => format!("{term}~{space}~{reason}"),
        false => format!("~np2:{}", encode_list(&[term, space, reason])),
    }
}

fn space(text: &str) -> Option<&'static str> {
    match text {
        "Xi" => Some("Xi"),
        "Yi" => Some("Yi"),
        _ => None,
    }
}

pub fn decode_negative(wire: &str) -> Option<(String, &'static str, String)> {
    match wire.strip_prefix("~np2:") {
        Some(hex) => {
            let parts = decode_list(hex)?;
            match parts.as_slice() {
                [term, tag, reason] => {
                    let tag = space(tag)?;
                    match encode_negative(term, tag, reason) == wire {
                        true => Some((term.to_owned(), tag, reason.to_owned())),
                        false => None,
                    }
                }
                _ => None,
            }
        }
        None => {
            match wire.len() <= MAX_BYTES {
                true => {}
                false => return None,
            }
            let mut parts = wire.split('~');
            let (term, tag, reason) = (parts.next()?, parts.next()?, parts.next()?);
            match parts.next().is_none() && legacy_negative(term, reason) {
                true => Some((term.to_owned(), space(tag)?, reason.to_owned())),
                false => None,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn safe_legacy_bytes_are_unchanged() {
        assert_eq!(encode_split(&["a".into(), "ë".into()], &[]), "xi:a,ë|yi:");
        assert_eq!(decode_split("xi:a,ë", "yi:"), Some((vec!["a".into(), "ë".into()], vec![])));
        assert_eq!(encode_negative("a", "Xi", "Reverse^-1"), "a~Xi~Reverse^-1");
        assert_eq!(decode_negative("a~Xi~Reverse^-1"), Some(("a".into(), "Xi", "Reverse^-1".into())));
    }

    #[test]
    fn exact_utf8_including_empty_control_and_normalization_distinctions() {
        let terms = ["jd_part2=54388.0,", "a|b", "a~b", "ë", "e\u{308}", " x ", "", "\0\r\n"]
            .map(str::to_owned).to_vec();
        let encoded = encode_split(&terms, &[]);
        let (x, y) = encoded.split_once('|').unwrap();
        assert_eq!(decode_split(x, y), Some((terms.clone(), vec![])));
        assert!(!encoded.contains('\n'));
        assert_eq!(encoded.matches('|').count(), 1);
        for term in terms {
            let reason = " cause~with|punctuation,\r\n ";
            assert_eq!(decode_negative(&encode_negative(&term, "Yi", reason)), Some((term, "Yi", reason.into())));
        }
    }

    #[test]
    fn split_rejects_mixed_versions_and_noncanonical_legacy() {
        let new = encode_split(&["a,".into()], &[]);
        let (x, y) = new.split_once('|').unwrap();
        assert!(decode_split(x, "yi:").is_none());
        assert!(decode_split("xi:a", y).is_none());
        for wire in ["xi:a,", "xi:a,,b", "xi: a", "xi:a|b"] {
            assert!(decode_split(wire, "yi:").is_none());
        }
        assert!(decode_split("xi3:00", "yi3:00").is_none());
        assert!(decode_legacy_terms(&",".repeat(MAX_ITEMS)).is_none());
    }

    #[test]
    fn length_hex_utf8_and_trailing_material_are_checked() {
        let valid = encode_list(&["a,"]);
        for end in 0..valid.len() {
            assert!(decode_list(&valid[..end]).is_none());
        }
        assert!(decode_list(&(valid.clone()+"00")).is_none());
        assert!(decode_list("ffffffffffffffff").is_none());
        let mut bad_length = valid.clone();
        bad_length.replace_range(16..32, "ffffffffffffffff");
        assert!(decode_list(&bad_length).is_none());
        let mut bad_utf8 = encode_list(&["a"]);
        bad_utf8.replace_range(32..34, "ff");
        assert!(decode_list(&bad_utf8).is_none());
        assert!(decode_list(&encode_list(&["\u{00ff}"]).to_uppercase()).is_none());
        assert!(decode_list(&"0".repeat(MAX_BYTES*2+2)).is_none());
    }

    #[test]
    fn declared_negative_v2_never_falls_back_on_malformed_payload() {
        assert!(decode_negative("~np2:nothex~Xi~reason").is_none());
        assert!(decode_negative("~np2:00").is_none());
        assert!(decode_negative(&format!("~np2:{}", encode_list(&["a~b", "Other", "reason"]))).is_none());
        assert!(decode_negative(&format!("~np2:{}", encode_list(&["a", "Xi", "reason"]))).is_none());
        assert!(decode_negative("a~Xi~reason~more").is_none());
        // The leading '~' marker cannot be a valid nonempty legacy term.
        assert_eq!(encode_negative("np2:a", "Xi", "reason"), "np2:a~Xi~reason");
        assert_eq!(decode_negative("np2:a~Xi~reason"), Some(("np2:a".into(), "Xi", "reason".into())));
    }
}
