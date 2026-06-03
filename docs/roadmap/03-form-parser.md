# Feature 3 — Form decoder (`form.rs`)

## Goal

Decode `application/x-www-form-urlencoded` request bodies in `no_std` with no
allocation, producing key/value pairs the generated `from_form` can consume.
Operates on `&str`/`&[u8]` and writes decoded output into caller-provided
heapless buffers.

## Subtasks

- [M1] Iterator over `key=value` pairs, splitting the body on `&` and each pair
  on the first `=`.
- [M1] Percent-decoding (`%XX`) and `+` → space, writing the decoded bytes into a
  caller-provided buffer (`heapless::Vec`/`String`) — no heap allocation.
- [M1] Error handling for malformed escapes (truncated `%`, non-hex digits),
  surfaced through `ParseError`.
- [M2] Edge cases: empty values, missing `=`, repeated keys, and an input
  size/limit guard to bound work on hostile input.

## Public surface / signatures

```rust
// form.rs (sketch)
pub struct FormPairs<'a> { /* ... */ }

impl<'a> Iterator for FormPairs<'a> {
    type Item = Result<(&'a str, FormValue<'a>), ParseError>;
}

/// Decode a single percent/`+`-encoded value into `out`, returning the slice.
pub fn decode_into<'b>(raw: &str, out: &'b mut [u8]) -> Result<&'b str, ParseError>;
```

(Exact shape to be finalized during implementation; the key constraint is
no-alloc + `ParseError` integration.)

## Test setup

Pure host unit tests, table-driven:

- normal pairs, multiple pairs
- percent-encoded and `+`-encoded values
- malformed escapes → error
- empty values, missing `=`, trailing `&`

Fully CI-testable without hardware; lives as `#[cfg(test)]` in `form.rs`.

## Open questions / risks

- Whether to expose borrowed slices (zero-copy when no decoding needed) vs.
  always decoding into a buffer. Prefer zero-copy fast path with a decode
  fallback.
- Maximum field/body sizes — coordinate with the HTTP buffer sizes in
  [04-http-server.md](04-http-server.md).
