//! Number parsing that accepts what the Dart prototype's `double.tryParse` / `int.tryParse` did.
//!
//! The grammars differ in small ways that matter for real files: Dart trims surrounding
//! whitespace, spells the specials `NaN`/`Infinity`, and lets an integer carry a `0x` prefix.
//! Rust's `str::parse` rejects whitespace and accepts lowercase `inf`/`nan`. Parsing through
//! here keeps "which files import" identical across the port.

pub(crate) fn parse_f64(raw: &str) -> Option<f64> {
    let s = raw.trim();
    match s {
        "NaN" | "+NaN" | "-NaN" => return Some(f64::NAN),
        "Infinity" | "+Infinity" => return Some(f64::INFINITY),
        "-Infinity" => return Some(f64::NEG_INFINITY),
        _ => {}
    }
    // Reject the Rust-only spellings (inf, infinity, nan in any case) Dart does not accept.
    if s.bytes()
        .any(|b| b.is_ascii_alphabetic() && !matches!(b, b'e' | b'E'))
    {
        return None;
    }
    s.parse::<f64>().ok()
}

pub(crate) fn parse_i64(raw: &str) -> Option<i64> {
    let s = raw.trim();
    let (negative, digits) = match s.as_bytes().first() {
        Some(b'-') => (true, &s[1..]),
        Some(b'+') => (false, &s[1..]),
        _ => (false, s),
    };
    let magnitude = if let Some(hex) = digits
        .strip_prefix("0x")
        .or_else(|| digits.strip_prefix("0X"))
    {
        if hex.is_empty() || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
            return None;
        }
        i128::from_str_radix(hex, 16).ok()?
    } else {
        if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        digits.parse::<i128>().ok()?
    };
    let value = if negative { -magnitude } else { magnitude };
    i64::try_from(value).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn doubles_follow_dart_grammar() {
        assert_eq!(parse_f64(" 1.5 "), Some(1.5));
        assert_eq!(parse_f64(".5"), Some(0.5));
        assert_eq!(parse_f64("5."), Some(5.0));
        assert_eq!(parse_f64("1e3"), Some(1000.0));
        assert_eq!(parse_f64("Infinity"), Some(f64::INFINITY));
        assert!(parse_f64("NaN").unwrap().is_nan());
        assert_eq!(parse_f64("inf"), None);
        assert_eq!(parse_f64("nan"), None);
        assert_eq!(parse_f64("abc"), None);
        assert_eq!(parse_f64(""), None);
    }

    #[test]
    fn integers_follow_dart_grammar() {
        assert_eq!(parse_i64(" 42 "), Some(42));
        assert_eq!(parse_i64("+7"), Some(7));
        assert_eq!(parse_i64("-3"), Some(-3));
        assert_eq!(parse_i64("0x1F"), Some(31));
        assert_eq!(parse_i64("1.0"), None);
        assert_eq!(parse_i64(""), None);
        assert_eq!(parse_i64("-"), None);
    }
}
