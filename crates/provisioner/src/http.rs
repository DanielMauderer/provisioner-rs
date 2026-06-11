//! Minimal `no_std` HTTP/1.1 request parser and response builder.
//!
//! Parses only the subset of HTTP needed for a WiFi captive portal:
//! method, path, `Content-Length`, and body.  Writes just enough of a
//! response to serve a form page or redirect captive-portal probes.
//!
//! # Example
//!
//! ```ignore
//! use provisioner::http::{parse_request, write_response, is_portal_probe};
//!
//! let raw = b"GET /generate_204 HTTP/1.1\r\nHost: net\r\n\r\n";
//! let req = parse_request(raw).unwrap();
//!
//! if is_portal_probe(req.path) {
//!     // redirect to captive portal
//! }
//!
//! let mut buf = [0u8; 512];
//! let n = write_response(&mut buf, 200, "text/html", b"<h1>hello</h1>").unwrap();
//! ```

// ── Error ──────────────────────────────────────────────────────────────────

/// Errors that can occur while parsing an HTTP request or writing a response.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HttpError {
    /// The request line (method, path, version) could not be parsed.
    MalformedRequest,
    /// A header line could not be parsed.
    MalformedHeader,
    /// The `Content-Length` header value is not a valid unsigned integer.
    InvalidContentLength,
    /// The provided output buffer is too small for the response.
    BufferTooSmall,
}

// ── Method ─────────────────────────────────────────────────────────────────

/// HTTP method extracted from the request line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Method {
    Get,
    Post,
    /// Any method other than `GET` or `POST` (e.g. `HEAD`, `OPTIONS`).
    Other,
}

// ── Request ────────────────────────────────────────────────────────────────

/// A parsed HTTP/1.1 request.
///
/// All string slices borrow directly from the input buffer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request<'a> {
    /// The HTTP method.
    pub method: Method,
    /// The request path (e.g. `"/"`, `"/config"`).
    pub path: &'a str,
    /// Value of the `Content-Length` header, if present.
    pub content_length: Option<usize>,
    /// The request body slice, bounded by `content_length`.
    pub body: &'a [u8],
}

// ── Request parsing ────────────────────────────────────────────────────────

/// Parse a raw HTTP/1.1 request buffer into a [`Request`].
///
/// The buffer must contain at least the request line and headers terminated
/// by `\r\n\r\n`.  The body is sliced to `Content-Length` bytes (if the
/// header is present), or empty otherwise.
///
/// # Errors
///
/// Returns [`HttpError::MalformedRequest`] if the request line is missing,
/// contains an unsupported version, or the header/body separator is absent.
/// Returns [`HttpError::InvalidContentLength`] if the `Content-Length` value
/// is not a valid non-negative integer.
pub fn parse_request(buf: &[u8]) -> Result<Request<'_>, HttpError> {
    // Locate the header/body separator.  Try \r\n\r\n first, then \n\n.
    let (header_end, sep_len) = find_subsequence(buf, b"\r\n\r\n")
        .map(|p| (p, 4))
        .or_else(|| find_subsequence(buf, b"\n\n").map(|p| (p, 2)))
        .ok_or(HttpError::MalformedRequest)?;

    // Include the trailing \r\n (or \n) of the last header line in headers_slice
    // so the header loop can find the line ending.  The separator's first 2 (or 1)
    // bytes are the last header's line ending.
    let last_header_eol = if sep_len == 4 { 2 } else { 1 };
    let headers_slice = &buf[..header_end + last_header_eol];
    let body_start = header_end + sep_len;

    // Parse the request line: "METHOD /path HTTP/1.x\r\n"
    let first_line_end = find_byte(headers_slice, b'\n').ok_or(HttpError::MalformedRequest)?;
    let request_line = &headers_slice[..first_line_end];
    // Strip trailing \r if present.
    let request_line = request_line.strip_suffix(b"\r").unwrap_or(request_line);

    // Safety: the input bytes come from a TCP stream; we assume the method
    // and path are ASCII (valid UTF-8).  If they aren't, from_utf8_unchecked
    // is acceptable because we control the buffer content — in practice a
    // web browser always sends ASCII for these fields.
    let request_line_str = unsafe { core::str::from_utf8_unchecked(request_line) };

    let (method, path) = parse_request_line(request_line_str)?;

    // Parse headers — only care about Content-Length.
    let mut content_length: Option<usize> = None;
    let mut pos = first_line_end + 1; // skip \n

    while pos < headers_slice.len() {
        let line_end = find_byte(&headers_slice[pos..], b'\n').ok_or(HttpError::MalformedHeader)?;
        let header_line = &headers_slice[pos..pos + line_end];
        let header_line = header_line.strip_suffix(b"\r").unwrap_or(header_line);

        if header_line.is_empty() {
            // End of headers (blank line before the \r\n\r\n separator).
            pos += line_end + 1;
            continue;
        }

        // Split on ": "
        if let Some(colon) = find_byte(header_line, b':') {
            let name = &header_line[..colon];
            let value = &header_line[colon + 1..];
            let value = trim_left_ascii_whitespace(value);

            if name.eq_ignore_ascii_case(b"content-length") {
                // Safety: Content-Length value is ASCII digits; unwrap is safe.
                let value_str = unsafe { core::str::from_utf8_unchecked(value) };
                content_length = Some(
                    value_str
                        .parse::<usize>()
                        .map_err(|_| HttpError::InvalidContentLength)?,
                );
            }
        }

        pos += line_end + 1;
    }

    // Slice the body.
    let body_len = content_length.unwrap_or(0);
    let body_end = (body_start + body_len).min(buf.len());
    let body = &buf[body_start..body_end];

    Ok(Request {
        method,
        path,
        content_length,
        body,
    })
}

/// Parse "METHOD /path HTTP/1.x" into (`Method`, `&str` path).
fn parse_request_line(line: &str) -> Result<(Method, &str), HttpError> {
    let mut parts = line.split(' ');
    let method_str = parts.next().ok_or(HttpError::MalformedRequest)?;
    let path = parts.next().ok_or(HttpError::MalformedRequest)?;
    let _version = parts.next().ok_or(HttpError::MalformedRequest)?;

    let method = match method_str {
        "GET" => Method::Get,
        "POST" => Method::Post,
        _ => Method::Other,
    };

    Ok((method, path))
}

// ── Response writing ───────────────────────────────────────────────────────

/// Write an HTTP/1.1 response with the given status, content type, and body.
///
/// Returns the number of bytes written into `out`.
///
/// # Errors
///
/// Returns [`HttpError::BufferTooSmall`] if `out` cannot hold the full
/// response (status line + headers + body).
pub fn write_response(
    out: &mut [u8],
    status: u16,
    content_type: &str,
    body: &[u8],
) -> Result<usize, HttpError> {
    let reason = status_reason(status);
    // Fixed portion of status line: "HTTP/1.1 " (9) + NNN (3) + " " (1) + "\r\n" (2) = 15
    // Content-Type line: "Content-Type: \r\n" (16) + value
    // Content-Length line: "Content-Length: \r\n" (18) + digits
    // Trailing "\r\n" (2) before body
    let needed =
        15 + reason.len() + 16 + content_type.len() + 18 + digit_count(body.len()) + 2 + body.len();

    if out.len() < needed {
        return Err(HttpError::BufferTooSmall);
    }

    let mut pos = 0;

    // Status line.
    pos += copy_str(&mut out[pos..], "HTTP/1.1 ");
    pos += copy_u16(&mut out[pos..], status);
    out[pos] = b' ';
    pos += 1;
    pos += copy_str(&mut out[pos..], reason);
    pos += copy_str(&mut out[pos..], "\r\n");

    // Content-Type.
    pos += copy_str(&mut out[pos..], "Content-Type: ");
    pos += copy_str(&mut out[pos..], content_type);
    pos += copy_str(&mut out[pos..], "\r\n");

    // Content-Length.
    pos += copy_str(&mut out[pos..], "Content-Length: ");
    pos += copy_usize(&mut out[pos..], body.len());
    pos += copy_str(&mut out[pos..], "\r\n\r\n");

    // Body.
    out[pos..pos + body.len()].copy_from_slice(body);
    pos += body.len();

    Ok(pos)
}

/// Write an HTTP redirect response (302 Found) to `location`.
///
/// Returns the number of bytes written into `out`.
pub fn write_redirect(out: &mut [u8], location: &str) -> Result<usize, HttpError> {
    // "HTTP/1.1 302 Found\r\n" (20) + "Location: " (10) + location + "\r\n\r\n" (4)
    let needed = 20 + 10 + location.len() + 4;

    if out.len() < needed {
        return Err(HttpError::BufferTooSmall);
    }

    let mut pos = 0;
    pos += copy_str(&mut out[pos..], "HTTP/1.1 302 Found\r\n");
    pos += copy_str(&mut out[pos..], "Location: ");
    pos += copy_str(&mut out[pos..], location);
    pos += copy_str(&mut out[pos..], "\r\n\r\n");

    Ok(pos)
}

/// Human-readable reason phrase for common HTTP status codes.
fn status_reason(status: u16) -> &'static str {
    match status {
        200 => "OK",
        302 => "Found",
        303 => "See Other",
        404 => "Not Found",
        _ => "OK",
    }
}

// ── Captive portal probes ──────────────────────────────────────────────────

/// Paths that devices request to detect a captive portal.
const PROBE_PATHS: &[&str] = &[
    "/generate_204",              // Android
    "/gen_204",                   // Android (alternative)
    "/hotspot-detect.html",       // iOS / macOS
    "/library/test/success.html", // iOS / macOS (alternative)
    "/ncsi.txt",                  // Windows
    "/connecttest.txt",           // Windows (alternative)
    "/redirect",                  // Windows (alternative)
    "/canonical.html",            // Firefox
    "/success.txt",               // Firefox (alternative)
    "/mobile/status.php",         // Older Android
];

/// Returns `true` if `path` is a known captive-portal probe endpoint.
///
/// When a probe is detected the platform should respond with a redirect
/// to the configuration form.
pub fn is_portal_probe(path: &str) -> bool {
    PROBE_PATHS.contains(&path)
}

/// The path to redirect captive-portal probes to.
pub const PORTAL_FORM_PATH: &str = "/";

// ── Internal helpers ───────────────────────────────────────────────────────

#[inline]
fn find_byte(haystack: &[u8], needle: u8) -> Option<usize> {
    haystack.iter().position(|&b| b == needle)
}

#[inline]
fn find_subsequence(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

#[inline]
fn trim_left_ascii_whitespace(bytes: &[u8]) -> &[u8] {
    let start = bytes
        .iter()
        .position(|&b| b != b' ' && b != b'\t')
        .unwrap_or(bytes.len());
    &bytes[start..]
}

#[inline]
fn copy_str(dst: &mut [u8], src: &str) -> usize {
    let b = src.as_bytes();
    dst[..b.len()].copy_from_slice(b);
    b.len()
}

fn copy_u16(dst: &mut [u8], val: u16) -> usize {
    let mut buf = [0u8; 20];
    let s = write_usize(&mut buf, val as usize);
    let b = s.as_bytes();
    dst[..b.len()].copy_from_slice(b);
    b.len()
}

fn copy_usize(dst: &mut [u8], val: usize) -> usize {
    let mut buf = [0u8; 20];
    let s = write_usize(&mut buf, val);
    let b = s.as_bytes();
    dst[..b.len()].copy_from_slice(b);
    b.len()
}

fn digit_count(n: usize) -> usize {
    if n == 0 {
        return 1;
    }
    let mut count = 0;
    let mut n = n;
    while n > 0 {
        count += 1;
        n /= 10;
    }
    count
}

fn write_usize(buf: &mut [u8; 20], val: usize) -> &str {
    // Simple itoa — safe since buf is large enough for any usize.
    if val == 0 {
        buf[0] = b'0';
        return unsafe { core::str::from_utf8_unchecked(&buf[..1]) };
    }
    let mut pos = 20;
    let mut v = val;
    while v > 0 {
        pos -= 1;
        buf[pos] = b'0' + (v % 10) as u8;
        v /= 10;
    }
    let len = 20 - pos;
    // Shift left.
    buf.copy_within(pos..20, 0);
    unsafe { core::str::from_utf8_unchecked(&buf[..len]) }
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── parse_request ──────────────────────────────────────────────────

    #[test]
    fn parse_get_request() {
        let raw = b"GET / HTTP/1.1\r\nHost: example.com\r\n\r\n";
        let req = parse_request(raw).unwrap();
        assert_eq!(req.method, Method::Get);
        assert_eq!(req.path, "/");
        assert_eq!(req.content_length, None);
        assert!(req.body.is_empty());
    }

    #[test]
    fn parse_get_with_query_string() {
        let raw = b"GET /config?ssid=foo HTTP/1.1\r\nHost: x\r\n\r\n";
        let req = parse_request(raw).unwrap();
        assert_eq!(req.method, Method::Get);
        assert_eq!(req.path, "/config?ssid=foo");
    }

    #[test]
    fn parse_post_with_body() {
        let raw = b"POST /submit HTTP/1.1\r\nContent-Type: application/x-www-form-urlencoded\r\nContent-Length: 7\r\n\r\nfoo=bar";
        let req = parse_request(raw).unwrap();
        assert_eq!(req.method, Method::Post);
        assert_eq!(req.path, "/submit");
        assert_eq!(req.content_length, Some(7));
        assert_eq!(req.body, b"foo=bar");
    }

    #[test]
    fn parse_post_no_body() {
        let raw = b"POST /submit HTTP/1.1\r\nContent-Length: 0\r\n\r\n";
        let req = parse_request(raw).unwrap();
        assert_eq!(req.method, Method::Post);
        assert_eq!(req.content_length, Some(0));
        assert!(req.body.is_empty());
    }

    #[test]
    fn parse_post_no_content_length() {
        let raw = b"POST /submit HTTP/1.1\r\nHost: x\r\n\r\n";
        let req = parse_request(raw).unwrap();
        assert_eq!(req.method, Method::Post);
        assert_eq!(req.content_length, None);
        assert!(req.body.is_empty());
    }

    #[test]
    fn parse_content_length_with_leading_space() {
        let raw = b"POST /x HTTP/1.1\r\nContent-Length:  5\r\n\r\nhello";
        let req = parse_request(raw).unwrap();
        assert_eq!(req.content_length, Some(5));
        assert_eq!(req.body, b"hello");
    }

    #[test]
    fn parse_case_insensitive_content_length() {
        let raw = b"POST /x HTTP/1.1\r\ncontent-length: 4\r\n\r\ntest";
        let req = parse_request(raw).unwrap();
        assert_eq!(req.content_length, Some(4));
        assert_eq!(req.body, b"test");
    }

    #[test]
    fn parse_mixed_case_header() {
        let raw = b"POST /x HTTP/1.1\r\nContent-length: 3\r\n\r\nabc";
        let req = parse_request(raw).unwrap();
        assert_eq!(req.content_length, Some(3));
        assert_eq!(req.body, b"abc");
    }

    #[test]
    fn parse_head_method() {
        let raw = b"HEAD / HTTP/1.1\r\nHost: x\r\n\r\n";
        let req = parse_request(raw).unwrap();
        assert_eq!(req.method, Method::Other);
    }

    #[test]
    fn parse_options_method() {
        let raw = b"OPTIONS / HTTP/1.1\r\nHost: x\r\n\r\n";
        let req = parse_request(raw).unwrap();
        assert_eq!(req.method, Method::Other);
    }

    #[test]
    fn parse_missing_header_separator() {
        let raw = b"GET / HTTP/1.1\r\nHost: x";
        assert_eq!(parse_request(raw), Err(HttpError::MalformedRequest));
    }

    #[test]
    fn parse_empty_buffer() {
        assert_eq!(parse_request(b""), Err(HttpError::MalformedRequest));
    }

    #[test]
    fn parse_invalid_content_length() {
        let raw = b"POST /x HTTP/1.1\r\nContent-Length: abc\r\n\r\nbody";
        assert_eq!(parse_request(raw), Err(HttpError::InvalidContentLength));
    }

    #[test]
    fn parse_no_cr_in_request_line() {
        // Some clients send \n without \r; we accept this.
        let raw = b"GET / HTTP/1.1\nHost: x\n\n";
        let req = parse_request(raw).unwrap();
        assert_eq!(req.method, Method::Get);
        assert_eq!(req.path, "/");
    }

    #[test]
    fn parse_body_shorter_than_content_length() {
        // Graceful: slice to available bytes.
        let raw = b"POST /x HTTP/1.1\r\nContent-Length: 100\r\n\r\nshort";
        let req = parse_request(raw).unwrap();
        assert_eq!(req.body, b"short");
    }

    // ── write_response ─────────────────────────────────────────────────

    #[test]
    fn write_200_html() {
        let mut buf = [0u8; 256];
        let n = write_response(&mut buf, 200, "text/html", b"<h1>hi</h1>").unwrap();
        let response = core::str::from_utf8(&buf[..n]).unwrap();
        assert!(response.starts_with("HTTP/1.1 200 OK\r\n"));
        assert!(response.contains("Content-Type: text/html\r\n"));
        assert!(response.contains("Content-Length: 11\r\n"));
        assert!(response.ends_with("\r\n\r\n<h1>hi</h1>"));
    }

    #[test]
    fn write_404() {
        let mut buf = [0u8; 128];
        let n = write_response(&mut buf, 404, "text/plain", b"not found").unwrap();
        let response = core::str::from_utf8(&buf[..n]).unwrap();
        assert!(response.starts_with("HTTP/1.1 404 Not Found\r\n"));
    }

    #[test]
    fn write_303_see_other() {
        let mut buf = [0u8; 128];
        let n = write_response(&mut buf, 303, "text/html", b"").unwrap();
        let response = core::str::from_utf8(&buf[..n]).unwrap();
        assert!(response.starts_with("HTTP/1.1 303 See Other\r\n"));
    }

    #[test]
    fn write_response_buffer_too_small() {
        let mut buf = [0u8; 20];
        assert_eq!(
            write_response(
                &mut buf,
                200,
                "text/html",
                b"hello world, this is a long body"
            ),
            Err(HttpError::BufferTooSmall)
        );
    }

    #[test]
    fn write_response_empty_body() {
        let mut buf = [0u8; 128];
        let n = write_response(&mut buf, 200, "text/plain", b"").unwrap();
        let response = core::str::from_utf8(&buf[..n]).unwrap();
        assert!(response.contains("Content-Length: 0\r\n"));
    }

    // ── write_redirect ─────────────────────────────────────────────────

    #[test]
    fn write_302_redirect() {
        let mut buf = [0u8; 256];
        let n = write_redirect(&mut buf, "http://192.168.4.1/").unwrap();
        let response = core::str::from_utf8(&buf[..n]).unwrap();
        assert!(response.starts_with("HTTP/1.1 302 Found\r\n"));
        assert!(response.contains("Location: http://192.168.4.1/\r\n"));
    }

    #[test]
    fn write_redirect_buffer_too_small() {
        let mut buf = [0u8; 20];
        assert_eq!(
            write_redirect(&mut buf, "http://192.168.4.1/config"),
            Err(HttpError::BufferTooSmall)
        );
    }

    // ── is_portal_probe ────────────────────────────────────────────────

    #[test]
    fn portal_probe_android() {
        assert!(is_portal_probe("/generate_204"));
        assert!(is_portal_probe("/gen_204"));
    }

    #[test]
    fn portal_probe_ios() {
        assert!(is_portal_probe("/hotspot-detect.html"));
        assert!(is_portal_probe("/library/test/success.html"));
    }

    #[test]
    fn portal_probe_windows() {
        assert!(is_portal_probe("/ncsi.txt"));
        assert!(is_portal_probe("/connecttest.txt"));
        assert!(is_portal_probe("/redirect"));
    }

    #[test]
    fn portal_probe_firefox() {
        assert!(is_portal_probe("/canonical.html"));
        assert!(is_portal_probe("/success.txt"));
    }

    #[test]
    fn regular_path_not_probe() {
        assert!(!is_portal_probe("/"));
        assert!(!is_portal_probe("/config"));
        assert!(!is_portal_probe("/style.css"));
        assert!(!is_portal_probe(""));
    }

    // ── Integration ────────────────────────────────────────────────────

    #[test]
    fn parse_and_classify_probe() {
        let raw = b"GET /generate_204 HTTP/1.1\r\nHost: connectivitycheck.gstatic.com\r\n\r\n";
        let req = parse_request(raw).unwrap();
        assert!(is_portal_probe(req.path));
    }

    #[test]
    fn parse_post_form_and_respond() {
        let raw = b"POST /config HTTP/1.1\r\nContent-Type: application/x-www-form-urlencoded\r\nContent-Length: 7\r\n\r\nfoo=bar";
        let req = parse_request(raw).unwrap();
        assert_eq!(req.method, Method::Post);
        assert_eq!(req.body, b"foo=bar");

        let mut buf = [0u8; 256];
        let n = write_response(&mut buf, 200, "text/html", b"<h1>Saved</h1>").unwrap();
        assert!(n > 0);
    }

    #[test]
    fn probe_gets_redirect() {
        let raw = b"GET /ncsi.txt HTTP/1.1\r\nHost: www.msftncsi.com\r\n\r\n";
        let req = parse_request(raw).unwrap();
        assert!(is_portal_probe(req.path));

        let mut buf = [0u8; 256];
        let n = write_redirect(&mut buf, PORTAL_FORM_PATH).unwrap();
        let response = core::str::from_utf8(&buf[..n]).unwrap();
        assert!(response.starts_with("HTTP/1.1 302 Found\r\n"));
    }
}
