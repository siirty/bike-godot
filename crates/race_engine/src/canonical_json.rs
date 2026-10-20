//! Canonical numeric JSON for versioned snapshots. This repository owns the byte format.
//! Finite doubles use round-tripping digits, preserve signed zero, retain an integral `.0`,
//! and use explicit exponent signs outside the fixed-notation window. Existing saved snapshots
//! keep their encoding; this is independent of any external runtime or generator.

use std::io;

/// The snapshot numeric contract for a finite or non-finite value.
///
/// Shortest round-tripping digits `d1…dk` with decimal exponent `n` (value = 0.d1…dk × 10ⁿ):
/// `k ≤ n ≤ 21` → digits, zeros, `.0`; `0 < n ≤ 21` → point inside; `−6 < n ≤ 0` → `0.000ddd`;
/// otherwise exponential with an explicit exponent sign.
pub fn format_number(value: f64) -> String {
    if value.is_nan() {
        return "NaN".to_owned();
    }
    if value.is_infinite() {
        return if value > 0.0 { "Infinity" } else { "-Infinity" }.to_owned();
    }
    if value == 0.0 {
        return if value.is_sign_negative() {
            "-0.0"
        } else {
            "0.0"
        }
        .to_owned();
    }
    let sign = if value < 0.0 { "-" } else { "" };
    let sci = format!("{:e}", value.abs());
    let (mantissa, exp) = sci.split_once('e').expect("{:e} has an exponent");
    let exp: i32 = exp.parse().expect("integer exponent");
    let digits: String = mantissa.chars().filter(|c| *c != '.').collect();
    let k = digits.len() as i32;
    let n = exp + 1;

    let body = if k <= n && n <= 21 {
        format!("{digits}{}.0", "0".repeat((n - k) as usize))
    } else if 0 < n && n <= 21 {
        format!("{}.{}", &digits[..n as usize], &digits[n as usize..])
    } else if -6 < n && n <= 0 {
        format!("0.{}{digits}", "0".repeat((-n) as usize))
    } else {
        let e = n - 1;
        let e_sign = if e < 0 { '-' } else { '+' };
        if k == 1 {
            format!("{digits}e{e_sign}{}", e.abs())
        } else {
            format!("{}.{}e{e_sign}{}", &digits[..1], &digits[1..], e.abs())
        }
    };
    format!("{sign}{body}")
}

/// A serde_json formatter that writes the canonical numeric representation. Non-finite values are never written:
/// every double in a snapshot was validated finite first.
#[derive(Debug, Clone, Copy, Default)]
pub struct CanonicalFormatter;

impl serde_json::ser::Formatter for CanonicalFormatter {
    fn write_f64<W: ?Sized + io::Write>(&mut self, writer: &mut W, value: f64) -> io::Result<()> {
        writer.write_all(format_number(value).as_bytes())
    }

    fn write_f32<W: ?Sized + io::Write>(&mut self, writer: &mut W, value: f32) -> io::Result<()> {
        self.write_f64(writer, f64::from(value))
    }
}

/// Serializes `value` compactly with [`CanonicalFormatter`].
pub fn to_canonical_json<T: serde::Serialize + ?Sized>(value: &T) -> String {
    let mut out = Vec::new();
    let mut ser = serde_json::Serializer::with_formatter(&mut out, CanonicalFormatter);
    value
        .serialize(&mut ser)
        .expect("in-memory serialization of plain data cannot fail");
    String::from_utf8(out).expect("serde_json writes UTF-8")
}

#[cfg(test)]
mod tests {
    use super::format_number as d;

    #[test]
    fn matches_format_number_to_string() {
        assert_eq!(d(8.0), "8.0");
        assert_eq!(d(-2.25), "-2.25");
        assert_eq!(d(0.1), "0.1");
        assert_eq!(d(1e21), "1e+21");
        assert_eq!(d(1e20), "100000000000000000000.0");
        assert_eq!(d(1.5e-7), "1.5e-7");
        assert_eq!(d(0.000001), "0.000001");
        assert_eq!(d(1e-7), "1e-7");
        assert_eq!(d(123456.789), "123456.789");
        assert_eq!(d(-0.0), "-0.0");
        assert_eq!(d(f64::NAN), "NaN");
    }
}
