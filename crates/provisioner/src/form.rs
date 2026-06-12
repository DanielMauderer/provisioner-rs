//! URL-encoded form body coder/decoder (`application/x-www-form-urlencoded`).
//!
//! Operates on `&str` with no allocation — suitable for `no_std` embedded use.
//!
//! # Example
//!
//! ```ignore
//! use provisioner::form::{FormPairs, decode_into, encode_into};
//!
//! let body = "ssid=MyWiFi&password=hello+world&use_dhcp=true";
//! let mut buf = [0u8; 64];
//!
//! for (key, raw_value) in FormPairs::new(body) {
//!     let value = decode_into(raw_value, &mut buf).unwrap();
//!     // use key and value ...
//! }
//!
//! let mut out = [0u8; 64];
//! let encoded = encode_into("hello world!", &mut out).unwrap();
//! assert_eq!(encoded, "hello+world%21");
//! ```

use crate::error::ParseError;

/// Iterator over raw `(key, value)` pairs in a URL-encoded form body.
///
/// Splits the body on `&` separators and each pair on the first `=`.
/// Values are returned undecoded — use [`decode_into`] to resolve
/// percent-encoding (`%XX`) and `+` → space.
///
/// Keys and values borrow directly from the input slice (zero-copy).
pub struct FormPairs<'a> {
    remaining: &'a str,
}

impl<'a> FormPairs<'a> {
    /// Create a new iterator over the form-encoded `body`.
    pub fn new(body: &'a str) -> Self {
        Self { remaining: body }
    }
}

impl<'a> Iterator for FormPairs<'a> {
    type Item = (&'a str, &'a str);

    fn next(&mut self) -> Option<Self::Item> {
        // Skip leading separators (also handles trailing `&` consumed below).
        self.remaining = self.remaining.trim_start_matches('&');

        if self.remaining.is_empty() {
            return None;
        }

        // Find the end of this pair (next `&` or end of string).
        let pair_end = self.remaining.find('&').unwrap_or(self.remaining.len());

        let pair = &self.remaining[..pair_end];

        // Advance past this pair. If there was a `&`, it's now at the front
        // and will be trimmed at the start of the next call.
        self.remaining = &self.remaining[pair_end..];

        match pair.split_once('=') {
            Some((key, value)) => Some((key, value)),
            None => Some((pair, "")),
        }
    }
}

/// Decode a single percent-encoded (`%XX`) / `+`→space value into `out`.
///
/// Returns a `&str` referencing the decoded portion of `out`.  
/// Literal bytes pass through unchanged; `+` becomes ` `; `%XX` is decoded
/// to the byte value of the two hex digits.
///
/// UTF-8 validity is checked before returning — if `%XX` decodes to a
/// non-UTF-8 byte sequence, [`ParseError::InvalidEncoding`] is returned.
///
/// # Errors
///
/// Returns [`ParseError::MalformedEncoding`] if:
/// - A `%` is followed by fewer than two hex digits.
/// - A `%` is followed by non-hexadecimal characters.
/// - The output buffer `out` is too small.
pub fn decode_into<'out>(raw: &str, out: &'out mut [u8]) -> Result<&'out str, ParseError> {
    let mut out_pos = 0;
    let bytes = raw.as_bytes();
    let mut i = 0;

    while i < bytes.len() {
        let b = bytes[i];
        i += 1;

        match b {
            b'+' => {
                if out_pos >= out.len() {
                    return Err(ParseError::MalformedEncoding);
                }
                out[out_pos] = b' ';
                out_pos += 1;
            }
            b'%' => {
                if i + 1 >= bytes.len() {
                    return Err(ParseError::MalformedEncoding);
                }
                let hi = bytes[i];
                let lo = bytes[i + 1];
                i += 2;

                let decoded = hex_val(hi)
                    .and_then(|h| hex_val(lo).map(|l| (h << 4) | l))
                    .ok_or(ParseError::MalformedEncoding)?;

                if out_pos >= out.len() {
                    return Err(ParseError::MalformedEncoding);
                }
                out[out_pos] = decoded;
                out_pos += 1;
            }
            _ => {
                if out_pos >= out.len() {
                    return Err(ParseError::MalformedEncoding);
                }
                out[out_pos] = b;
                out_pos += 1;
            }
        }
    }

    // `%XX` can decode to non-UTF-8 byte sequences; validate before returning.
    let decoded = core::str::from_utf8(&out[..out_pos]).map_err(|_| ParseError::InvalidEncoding)?;
    Ok(decoded)
}

/// Encode a plaintext value into URL-encoded form (`+` for space, `%XX` for
/// reserved bytes), writing into `out`.
///
/// The encoded form is compatible with [`decode_into`]: spaces become `+`, the
/// bytes `&`, `=`, `%`, `+`, and any non-ASCII / control byte are percent-encoded.
///
/// Returns the encoded substring of `out`.
///
/// # Errors
///
/// Returns [`ParseError::MalformedEncoding`] if `out` is too small to hold the
/// encoded result.
pub fn encode_into<'out>(plain: &str, out: &'out mut [u8]) -> Result<&'out str, ParseError> {
    let mut out_pos = 0;

    for &b in plain.as_bytes() {
        match b {
            b' ' => {
                if out_pos >= out.len() {
                    return Err(ParseError::MalformedEncoding);
                }
                out[out_pos] = b'+';
                out_pos += 1;
            }
            b'&' | b'=' | b'%' | b'+' => {
                if out_pos + 2 >= out.len() {
                    return Err(ParseError::MalformedEncoding);
                }
                let encoded = percent_encode(b);
                out[out_pos] = encoded[0];
                out[out_pos + 1] = encoded[1];
                out[out_pos + 2] = encoded[2];
                out_pos += 3;
            }
            b if b.is_ascii_alphanumeric() || b"~_-./:?#[]@!$'()*,".contains(&b) => {
                if out_pos >= out.len() {
                    return Err(ParseError::MalformedEncoding);
                }
                out[out_pos] = b;
                out_pos += 1;
            }
            _ => {
                if out_pos + 2 >= out.len() {
                    return Err(ParseError::MalformedEncoding);
                }
                let encoded = percent_encode(b);
                out[out_pos] = encoded[0];
                out[out_pos + 1] = encoded[1];
                out[out_pos + 2] = encoded[2];
                out_pos += 3;
            }
        }
    }

    // SAFETY: we only wrote ASCII bytes (`+`, `%`, hex digits, and unreserved).
    Ok(unsafe { core::str::from_utf8_unchecked(&out[..out_pos]) })
}

/// Encode a `key=value` pair into `out`, separating with `&` if `out` already
/// contains a previous pair.
///
/// This is a convenience for serializing multiple form fields into a single
/// buffer, e.g. for flash storage round-trips.
pub fn encode_pair_into(
    out: &mut [u8],
    used: &mut usize,
    key: &str,
    value: &str,
) -> Result<(), ParseError> {
    if *used > 0 {
        if *used >= out.len() {
            return Err(ParseError::MalformedEncoding);
        }
        out[*used] = b'&';
        *used += 1;
    }

    let key_encoded = encode_into(key, &mut out[*used..])?;
    *used += key_encoded.len();

    if *used >= out.len() {
        return Err(ParseError::MalformedEncoding);
    }
    out[*used] = b'=';
    *used += 1;

    let value_encoded = encode_into(value, &mut out[*used..])?;
    *used += value_encoded.len();

    Ok(())
}

#[inline]
fn percent_encode(b: u8) -> [u8; 3] {
    [b'%', hex_digit(b >> 4), hex_digit(b & 0x0F)]
}

#[inline]
fn hex_digit(n: u8) -> u8 {
    match n {
        0..=9 => b'0' + n,
        10..=15 => b'A' + (n - 10),
        _ => unreachable!(),
    }
}

/// Convert a hex character (`0`-`9`, `a`-`f`, `A`-`F`) to its 4-bit value.
#[inline]
fn hex_val(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── FormPairs ──────────────────────────────────────────────────────────

    #[test]
    fn single_pair() {
        let mut iter = FormPairs::new("ssid=MyWiFi");
        assert_eq!(iter.next(), Some(("ssid", "MyWiFi")));
        assert_eq!(iter.next(), None);
    }

    #[test]
    fn multiple_pairs() {
        let mut iter = FormPairs::new("a=1&b=2&c=3");
        assert_eq!(iter.next(), Some(("a", "1")));
        assert_eq!(iter.next(), Some(("b", "2")));
        assert_eq!(iter.next(), Some(("c", "3")));
        assert_eq!(iter.next(), None);
    }

    #[test]
    fn empty_body() {
        let mut iter = FormPairs::new("");
        assert_eq!(iter.next(), None);
    }

    #[test]
    fn empty_value() {
        let mut iter = FormPairs::new("key=");
        assert_eq!(iter.next(), Some(("key", "")));
        assert_eq!(iter.next(), None);
    }

    #[test]
    fn missing_equals_is_key_with_empty_value() {
        let mut iter = FormPairs::new("flag");
        assert_eq!(iter.next(), Some(("flag", "")));
        assert_eq!(iter.next(), None);
    }

    #[test]
    fn empty_key_with_value() {
        let mut iter = FormPairs::new("=value");
        assert_eq!(iter.next(), Some(("", "value")));
        assert_eq!(iter.next(), None);
    }

    #[test]
    fn trailing_separator() {
        let mut iter = FormPairs::new("a=1&b=2&");
        assert_eq!(iter.next(), Some(("a", "1")));
        assert_eq!(iter.next(), Some(("b", "2")));
        assert_eq!(iter.next(), None);
    }

    #[test]
    fn leading_separator() {
        let mut iter = FormPairs::new("&a=1");
        assert_eq!(iter.next(), Some(("a", "1")));
        assert_eq!(iter.next(), None);
    }

    #[test]
    fn consecutive_separators() {
        let mut iter = FormPairs::new("a=1&&b=2");
        assert_eq!(iter.next(), Some(("a", "1")));
        assert_eq!(iter.next(), Some(("b", "2")));
        assert_eq!(iter.next(), None);
    }

    #[test]
    fn value_contains_equals() {
        let mut iter = FormPairs::new("op=1+1=2");
        assert_eq!(iter.next(), Some(("op", "1+1=2")));
        assert_eq!(iter.next(), None);
    }

    #[test]
    fn iterator_does_not_decode() {
        let mut iter = FormPairs::new("msg=hello%20world+again");
        let (key, raw) = iter.next().unwrap();
        assert_eq!(key, "msg");
        assert_eq!(raw, "hello%20world+again");
        assert_eq!(iter.next(), None);
    }

    // ── decode_into ────────────────────────────────────────────────────────

    #[test]
    fn decode_plain() {
        let mut buf = [0u8; 32];
        let result = decode_into("hello", &mut buf).unwrap();
        assert_eq!(result, "hello");
    }

    #[test]
    fn decode_plus_to_space() {
        let mut buf = [0u8; 32];
        let result = decode_into("hello+world", &mut buf).unwrap();
        assert_eq!(result, "hello world");
    }

    #[test]
    fn decode_multiple_plus() {
        let mut buf = [0u8; 32];
        let result = decode_into("a+b+c", &mut buf).unwrap();
        assert_eq!(result, "a b c");
    }

    #[test]
    fn decode_percent_escape_space() {
        let mut buf = [0u8; 32];
        let result = decode_into("hello%20world", &mut buf).unwrap();
        assert_eq!(result, "hello world");
    }

    #[test]
    fn decode_percent_escape_special_chars() {
        let mut buf = [0u8; 64];
        let result = decode_into("a%3Db%26c%3Dd", &mut buf).unwrap();
        assert_eq!(result, "a=b&c=d");
    }

    #[test]
    fn decode_mixed_plus_and_percent() {
        let mut buf = [0u8; 64];
        let result = decode_into("key%3Dval+ue", &mut buf).unwrap();
        assert_eq!(result, "key=val ue");
    }

    #[test]
    fn decode_all_percent() {
        let mut buf = [0u8; 32];
        let result = decode_into("%48%65%6C%6C%6F", &mut buf).unwrap();
        assert_eq!(result, "Hello");
    }

    #[test]
    fn decode_lowercase_hex() {
        let mut buf = [0u8; 32];
        let result = decode_into("%2fusr%2fbin", &mut buf).unwrap();
        assert_eq!(result, "/usr/bin");
    }
    #[test]
    fn decode_percent_null_byte_valid_utf8() {
        // %00 decodes to 0x00 (U+0000 NULL), which is valid UTF-8.
        let mut buf = [0u8; 32];
        let result = decode_into("a%00b", &mut buf).unwrap();
        assert_eq!(result, "a\0b");
    }

    #[test]
    fn decode_invalid_utf8_rejected() {
        // %FF decodes to 0xFF which is never valid UTF-8.
        let mut buf = [0u8; 32];
        assert_eq!(
            decode_into("a%FFb", &mut buf),
            Err(ParseError::InvalidEncoding)
        );
    }

    #[test]
    fn decode_truncated_percent_at_end() {
        let mut buf = [0u8; 32];
        assert_eq!(
            decode_into("bad%", &mut buf),
            Err(ParseError::MalformedEncoding)
        );
    }

    #[test]
    fn decode_truncated_percent_one_hex() {
        let mut buf = [0u8; 32];
        assert_eq!(
            decode_into("bad%2", &mut buf),
            Err(ParseError::MalformedEncoding)
        );
    }

    #[test]
    fn decode_non_hex_percent() {
        let mut buf = [0u8; 32];
        assert_eq!(
            decode_into("bad%GG", &mut buf),
            Err(ParseError::MalformedEncoding)
        );
    }

    #[test]
    fn decode_non_hex_percent_lower() {
        let mut buf = [0u8; 32];
        assert_eq!(
            decode_into("bad%2g", &mut buf),
            Err(ParseError::MalformedEncoding)
        );
    }

    #[test]
    fn decode_buffer_too_small() {
        let mut buf = [0u8; 3];
        assert_eq!(
            decode_into("hello", &mut buf),
            Err(ParseError::MalformedEncoding)
        );
    }

    #[test]
    fn decode_buffer_too_small_percent() {
        let mut buf = [0u8; 2];
        assert_eq!(
            decode_into("a%20b", &mut buf),
            Err(ParseError::MalformedEncoding)
        );
    }

    #[test]
    fn decode_empty_string() {
        let mut buf = [0u8; 1];
        let result = decode_into("", &mut buf).unwrap();
        assert_eq!(result, "");
    }

    #[test]
    fn decode_only_plus() {
        let mut buf = [0u8; 32];
        let result = decode_into("+++", &mut buf).unwrap();
        assert_eq!(result, "   ");
    }

    // ── Integration: FormPairs + decode_into ───────────────────────────────

    #[test]
    fn iterate_and_decode_each_pair() {
        let body = "ssid=My+WiFi&password=s3cr3t%21&use_dhcp=true";

        // Decode each value and compare against expected plaintext.
        let expected = ["My WiFi", "s3cr3t!", "true"];
        let mut buf = [0u8; 64];
        let mut i = 0;

        for (_key, raw_value) in FormPairs::new(body) {
            let decoded = decode_into(raw_value, &mut buf).unwrap();
            assert_eq!(decoded, expected[i], "pair {i}");
            i += 1;
        }
        assert_eq!(i, 3);
    }

    // ── encode_into ────────────────────────────────────────────────────────

    #[test]
    fn encode_plain() {
        let mut buf = [0u8; 32];
        assert_eq!(encode_into("hello", &mut buf).unwrap(), "hello");
    }

    #[test]
    fn encode_space_to_plus() {
        let mut buf = [0u8; 32];
        assert_eq!(encode_into("hello world", &mut buf).unwrap(), "hello+world");
    }

    #[test]
    fn encode_ampersand() {
        let mut buf = [0u8; 32];
        assert_eq!(encode_into("a&b", &mut buf).unwrap(), "a%26b");
    }

    #[test]
    fn encode_equals() {
        let mut buf = [0u8; 32];
        assert_eq!(encode_into("a=b", &mut buf).unwrap(), "a%3Db");
    }

    #[test]
    fn encode_percent() {
        let mut buf = [0u8; 32];
        assert_eq!(encode_into("100%", &mut buf).unwrap(), "100%25");
    }

    #[test]
    fn encode_plus() {
        let mut buf = [0u8; 32];
        assert_eq!(encode_into("1+1", &mut buf).unwrap(), "1%2B1");
    }

    #[test]
    fn encode_special_chars() {
        let mut buf = [0u8; 64];
        assert_eq!(encode_into("a=b&c+d", &mut buf).unwrap(), "a%3Db%26c%2Bd");
    }

    #[test]
    fn encode_non_ascii() {
        let mut buf = [0u8; 32];
        assert_eq!(encode_into("héllo", &mut buf).unwrap(), "h%C3%A9llo");
    }

    #[test]
    fn encode_empty_string() {
        let mut buf = [0u8; 1];
        assert_eq!(encode_into("", &mut buf).unwrap(), "");
    }

    #[test]
    fn encode_buffer_too_small() {
        let mut buf = [0u8; 2];
        assert_eq!(
            encode_into("hello", &mut buf),
            Err(ParseError::MalformedEncoding)
        );
    }

    #[test]
    fn encode_buffer_too_small_for_percent() {
        let mut buf = [0u8; 2];
        assert_eq!(
            encode_into("a&b", &mut buf),
            Err(ParseError::MalformedEncoding)
        );
    }

    // ── Round-trip: encode → decode ─────────────────────────────────────────

    #[test]
    fn round_trip_simple() {
        let plain = "ssid=My WiFi&password=s3cr3t!";
        let mut enc = [0u8; 128];
        let encoded = encode_into(plain, &mut enc).unwrap();

        let mut dec = [0u8; 128];
        let decoded = decode_into(encoded, &mut dec).unwrap();
        assert_eq!(decoded, plain);
    }

    #[test]
    fn round_trip_via_pairs() {
        let pairs = [("ssid", "My WiFi"), ("password", "p@ss+w0rd=&%")];
        let mut buf = [0u8; 256];
        let mut used = 0;

        for (k, v) in &pairs {
            encode_pair_into(&mut buf, &mut used, k, v).unwrap();
        }

        let body = core::str::from_utf8(&buf[..used]).unwrap();
        let mut dec_buf = [0u8; 128];
        let mut i = 0;

        for (key, raw_value) in FormPairs::new(body) {
            let value = decode_into(raw_value, &mut dec_buf).unwrap();
            assert_eq!(key, pairs[i].0);
            assert_eq!(value, pairs[i].1);
            i += 1;
        }

        assert_eq!(i, pairs.len());
    }

    // ── encode_pair_into ───────────────────────────────────────────────────────

    #[test]
    fn encode_pair_single() {
        let mut buf = [0u8; 64];
        let mut used = 0;
        encode_pair_into(&mut buf, &mut used, "ssid", "My WiFi").unwrap();
        assert_eq!(core::str::from_utf8(&buf[..used]).unwrap(), "ssid=My+WiFi");
    }

    #[test]
    fn encode_pair_multiple() {
        let mut buf = [0u8; 128];
        let mut used = 0;
        encode_pair_into(&mut buf, &mut used, "ssid", "My WiFi").unwrap();
        encode_pair_into(&mut buf, &mut used, "password", "s3cr3t!").unwrap();
        assert_eq!(
            core::str::from_utf8(&buf[..used]).unwrap(),
            "ssid=My+WiFi&password=s3cr3t!"
        );
    }

    #[test]
    fn encode_pair_buffer_too_small() {
        let mut buf = [0u8; 4];
        let mut used = 0;
        assert_eq!(
            encode_pair_into(&mut buf, &mut used, "ssid", "My WiFi"),
            Err(ParseError::MalformedEncoding)
        );
    }
}
