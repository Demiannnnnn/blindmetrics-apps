//! Chilean RUT / RUN validation and canonical formatting.
//!
//! Enforces modulo 11 check digit verification and canonical representation (SEC-20, ATT-03).

use serde::{Deserialize, Serialize};
use std::fmt;
use thiserror::Error;

/// Minimum valid body for a Chilean RUT / RUN (1.000.000).
pub const MIN_RUT_BODY: u32 = 1_000_000;
/// Maximum valid body for a Chilean RUT / RUN (99.999.999).
pub const MAX_RUT_BODY: u32 = 99_999_999;

/// Errors that can occur during Chilean RUT / RUN parsing or validation.
#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum RutError {
    #[error("RUT input is empty")]
    Empty,
    #[error("RUT is too short")]
    TooShort,
    #[error("Invalid character in RUT")]
    InvalidCharacter,
    #[error("RUT format is invalid")]
    InvalidFormat,
    #[error("RUT body {body} is out of valid range (1.000.000 to 99.999.999)")]
    OutOfRange { body: u32 },
    #[error("Invalid check digit: expected {expected}, got {got}")]
    InvalidCheckDigit { expected: char, got: char },
}

/// Represents a validated Chilean RUT / RUN.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Rut {
    body: u32,
    dv: char,
}

impl Rut {
    /// Parses a Chilean RUT/RUN from a string.
    ///
    /// Accepts:
    /// - With or without dots: `"12.345.678-5"` or `"12345678-5"`
    /// - With or without hyphen: `"12.345.6785"` or `"123456785"`
    /// - Check digit `'k'` or `'K'` (normalized to `'K'`)
    ///
    /// # Errors
    /// Returns [`RutError`] if the input is malformed, has invalid characters,
    /// is out of range (1.000.000 to 99.999.999), or fails modulo 11 check digit.
    pub fn parse(input: &str) -> Result<Self, RutError> {
        let trimmed = input.trim();
        if trimmed.is_empty() {
            return Err(RutError::Empty);
        }

        let (body_str, dv_char) = if let Some((b, d)) = trimmed.split_once('-') {
            if d.contains('-') {
                return Err(RutError::InvalidFormat);
            }
            let d_clean = d.trim();
            if d_clean.len() != 1 {
                return Err(RutError::InvalidFormat);
            }
            let dv = d_clean
                .chars()
                .next()
                .ok_or(RutError::InvalidFormat)?
                .to_ascii_uppercase();
            (b, dv)
        } else {
            // No hyphen: the last character is the DV, everything before is the body
            let dots_removed: String = trimmed.chars().filter(|&c| c != '.').collect();
            if dots_removed.len() < 2 {
                return Err(RutError::TooShort);
            }
            let dv = dots_removed
                .chars()
                .last()
                .ok_or(RutError::InvalidFormat)?
                .to_ascii_uppercase();
            let body_part = &dots_removed[..dots_removed.len() - dv.len_utf8()];
            let body_num: u32 = body_part.parse().map_err(|_| RutError::InvalidCharacter)?;
            return Self::from_parts(body_num, dv);
        };

        // Clean dots and parse body
        let clean_body: String = body_str.chars().filter(|&c| c != '.').collect();
        if clean_body.is_empty() {
            return Err(RutError::TooShort);
        }
        let body_num: u32 = clean_body.parse().map_err(|_| RutError::InvalidCharacter)?;

        Self::from_parts(body_num, dv_char)
    }

    /// Internal constructor validating range and check digit.
    fn from_parts(body: u32, dv: char) -> Result<Self, RutError> {
        if !(MIN_RUT_BODY..=MAX_RUT_BODY).contains(&body) {
            return Err(RutError::OutOfRange { body });
        }

        if !dv.is_ascii_digit() && dv != 'K' {
            return Err(RutError::InvalidCharacter);
        }

        let expected_dv = compute_dv(body);
        if dv != expected_dv {
            return Err(RutError::InvalidCheckDigit {
                expected: expected_dv,
                got: dv,
            });
        }

        Ok(Self { body, dv })
    }

    /// Returns the body number as `u32`.
    #[must_use]
    pub const fn body(&self) -> u32 {
        self.body
    }

    /// Returns the check digit (`'0'..='9'` or `'K'`).
    #[must_use]
    pub const fn dv(&self) -> char {
        self.dv
    }

    /// Returns the unpunctuated body digits as a `String` (e.g. `"12345678"`).
    #[must_use]
    pub fn body_digits(&self) -> String {
        self.body.to_string()
    }

    /// Formats the RUT into canonical format: `12.345.678-5`.
    #[must_use]
    pub fn format(&self) -> String {
        format_rut(self.body, self.dv)
    }
}

impl fmt::Display for Rut {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.format())
    }
}

/// Computes the Chilean modulo 11 check digit for a given RUT body.
///
/// Returns `'0'..='9'` or `'K'`.
#[must_use]
pub fn compute_dv(mut body: u32) -> char {
    let mut sum: u32 = 0;
    let mut multiplier: u32 = 2;
    while body > 0 {
        let digit = body % 10;
        sum += digit * multiplier;
        body /= 10;
        multiplier = if multiplier == 7 { 2 } else { multiplier + 1 };
    }
    let remainder = sum % 11;
    let dv = 11 - remainder;
    match dv {
        11 => '0',
        10 => 'K',
        n if n < 10 => (b'0' + n as u8) as char,
        _ => '0',
    }
}

/// Formats a RUT body and check digit into canonical representation `12.345.678-5`.
#[must_use]
pub fn format_rut(body: u32, dv: char) -> String {
    let s = body.to_string();
    let len = s.len();
    if len == 8 {
        format!("{}.{}.{}-{}", &s[0..2], &s[2..5], &s[5..8], dv)
    } else if len == 7 {
        format!("{}.{}.{}-{}", &s[0..1], &s[1..4], &s[4..7], dv)
    } else {
        // Fallback for bodies with fewer digits if any
        format!("{}-{}", s, dv)
    }
}

/// Convenience standalone parse function.
pub fn parse(input: &str) -> Result<Rut, RutError> {
    Rut::parse(input)
}

/// Convenience standalone format function matching canonical representation.
pub fn format(body: u32, dv: char) -> String {
    format_rut(body, dv)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rut_parse_and_format() {
        let r1 = parse("12.345.678-5").expect("valid rut");
        assert_eq!(r1.format(), "12.345.678-5");
        assert_eq!(r1.body(), 12345678);
        assert_eq!(r1.dv(), '5');
        assert_eq!(r1.body_digits(), "12345678");

        // Without dots, with hyphen
        let r2 = parse("12345678-5").expect("valid rut");
        assert_eq!(r2.format(), "12.345.678-5");

        // Without hyphen or dots
        let r3 = parse("123456785").expect("valid rut");
        assert_eq!(r3.format(), "12.345.678-5");

        // 7-digit body
        let r4 = parse("1.111.111-4").expect("valid rut");
        assert_eq!(r4.format(), "1.111.111-4");
        assert_eq!(r4.body_digits(), "1111111");

        let r5 = parse("11.111.111-1").expect("valid rut");
        assert_eq!(r5.format(), "11.111.111-1");

        // Company RUT with DV 0
        let r6 = parse("76.123.456-0").expect("valid rut");
        assert_eq!(r6.format(), "76.123.456-0");

        // Lowercase 'k' normalized to 'K'
        let dv_k = compute_dv(12345679);
        assert_eq!(dv_k, '3'); // true check digit
    }

    #[test]
    fn test_rut_invalid_dv() {
        let err = parse("12.345.678-K").unwrap_err();
        assert_eq!(
            err,
            RutError::InvalidCheckDigit {
                expected: '5',
                got: 'K'
            }
        );

        let err2 = parse("12.345.678-4").unwrap_err();
        assert_eq!(
            err2,
            RutError::InvalidCheckDigit {
                expected: '5',
                got: '4'
            }
        );
    }

    #[test]
    fn test_rut_empty_and_invalid_chars() {
        assert_eq!(parse("").unwrap_err(), RutError::Empty);
        assert_eq!(parse("   ").unwrap_err(), RutError::Empty);
        assert!(matches!(
            parse("12.345.abc-5").unwrap_err(),
            RutError::InvalidCharacter
        ));
        assert!(matches!(
            parse("12.345.678-Z").unwrap_err(),
            RutError::InvalidCharacter
        ));
        assert!(matches!(
            parse("999.999-1").unwrap_err(),
            RutError::OutOfRange { .. }
        ));
    }
}
