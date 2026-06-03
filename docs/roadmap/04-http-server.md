# Feature 4 — HTTP/1.1 server primitives (`http.rs`)

## Goal

A minimal, `no_std`/no-alloc HTTP/1.1 request parser and response builder
operating over `&[u8]`. Enough to serve the portal: handle `GET` (return the
form) and `POST` (extract the body for form parsing), plus the captive-portal
probe endpoints phones use to detect a portal.

## Subtasks

- [M1] Parse the request line (method, path, version) and headers; extract
  `Content-Length` and the body slice.
- [M1] Response builder for the responses the portal needs — `200` (HTML body),
  `302`/`303` redirect, `404` — writing into a caller-provided heapless buffer.
- [M1] Captive-portal probe endpoints returning redirects to the form, e.g.
  `/generate_204` (Android), `/hotspot-detect.html` (iOS/macOS),
  `/ncsi.txt` (Windows), and a catch-all redirect.
- [M2] Robustness: partial-read / multi-segment request assembly, header count
  and size limits, `Connection: keep-alive` vs `close` handling, and max-body
  guards.

## Public surface / signatures

```rust
// http.rs (sketch)
pub enum Method { Get, Post, Other }

pub struct Request<'a> {
    pub method: Method,
    pub path: &'a str,
    pub content_length: Option<usize>,
    pub body: &'a [u8],
}

pub fn parse_request(buf: &[u8]) -> Result<Request<'_>, HttpError>;

pub fn write_response(
    out: &mut [u8],
    status: u16,
    content_type: &str,
    body: &[u8],
) -> Result<usize, HttpError>;
```

## Test setup

Host unit tests feeding raw byte buffers and asserting:

- parsed method / path / content-length / body for `GET` and `POST`
- correct status line + headers from the response builder
- probe endpoints map to redirects

Hardware-independent; `#[cfg(test)]` in `http.rs`.

## Open questions / risks

- Buffer sizing: pick request/response buffer capacities that fit a typical
  config form yet stay small for the device. Coordinate with
  [03-form-parser.md](03-form-parser.md) and the platform loop.
- How strict to be on HTTP conformance — keep it pragmatic for the captive
  portal use case rather than a general-purpose server.
