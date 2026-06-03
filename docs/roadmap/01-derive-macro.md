# Feature 1 — `#[derive(Provision)]` macro

## Goal

Parse the annotated struct at compile time and emit everything the runtime
needs: the `ProvisionConfig` trait impl (HTML page, form parser, storage
round-trip). The macro is the single entry point users interact with, so it
also owns the helper-attribute surface for customization.

This crate must remain its own crate (`provisioner-macro`) because Rust requires
proc-macros to live in a dedicated `proc-macro = true` crate. It runs on the
host with full `std` available.

## Subtasks

- [M1] Define the `ProvisionConfig` trait in the main `provisioner` crate and
  re-export it from `lib.rs`. Surface:
  - `const HTML: &'static str`
  - `fn from_form(body: &[u8]) -> Result<Self, ParseError>`
  - `fn to_bytes(&self, buf: &mut [u8]) -> Result<usize, ParseError>`
  - `fn from_bytes(buf: &[u8]) -> Result<Self, ParseError>`

  **Byte boundary:** `from_form` takes `&[u8]` so it composes directly with
  `http::Request.body: &[u8]` (Feature 4) and the byte-oriented form decoder
  (Feature 3) — no caller-side `&str` conversion. The whole request → parse
  pipeline stays `&[u8]`. UTF-8 validation happens inside `from_form` (and/or
  per-value in the decoder); non-UTF-8 input maps to a dedicated
  `ParseError::InvalidEncoding` variant (a new variant to add to `error.rs`).
- [M1] Parse named-struct fields with `syn` (`DeriveInput` → `Data::Struct` →
  `Fields::Named`). Reject enums, unions, tuple structs, and unit structs with a
  clear, span-accurate compile error (`syn::Error::new_spanned(...).to_compile_error()`).
- [M1] Generate `from_form`: validate the body is UTF-8 (→
  `ParseError::InvalidEncoding` on failure), iterate the decoded key/value pairs
  (Feature 3), match each field name, call `.parse::<FieldType>()`, mapping parse
  failures to `ParseError::InvalidValue(field)` and absent keys to
  `ParseError::MissingField(field)`.
- [M1] Generate the `HTML` const by delegating to the HTML-builder logic
  (Feature 2). All concatenation happens at macro-expansion time so the emitted
  value is a single `&'static str`.
- [M1] Generate `to_bytes`/`from_bytes` consistent with the storage format
  (Feature 6).
- [M1] Parse container-level helper attributes:
  `#[provision(css = ..., js = ..., header = ..., footer = ...)]` (each a
  `&str` expression, typically `include_str!`).
- [M1] Parse field-level attributes that cover the **special input cases**
  (rather than inferring them from the field name):
  - `#[provision(default = ...)]` — initial value rendered into the input.
  - `#[provision(secret)]` — render the input as `type="password"`.
  - `#[provision(input_type = "...")]` — explicit override of the inferred input
    type (e.g. `"email"`, `"number"`, `"password"`).
- [M2] Richer field-level attributes: `label`, `placeholder`, `id`, `class`,
  and `validate`/range bounds.
- [M2] Broaden field-type support and provide a `FromStr`-bound generic
  fallback; document the field contract (every field type must implement
  `core::str::FromStr`).

## Public surface / signatures

```rust
// in provisioner (main crate)
pub trait ProvisionConfig: Sized {
    const HTML: &'static str;
    fn from_form(body: &[u8]) -> Result<Self, ParseError>;
    fn to_bytes(&self, buf: &mut [u8]) -> Result<usize, ParseError>;
    fn from_bytes(buf: &[u8]) -> Result<Self, ParseError>;
}
```

```rust
// user code
#[derive(Provision)]
#[provision(css = include_str!("theme.css"), header = include_str!("logo.html"))]
struct MyConfig {
    ssid: heapless::String<32>,
    #[provision(secret, default = "")]
    password: heapless::String<64>,
    use_dhcp: bool,
}
```

## Test setup

- `trybuild` compile-fail tests for: non-struct input, unsupported field type,
  malformed/unknown attribute. Lives in `crates/provisioner-macro/tests/`.
- `macrotest` (or a `cargo expand` snapshot) verifying the generated `HTML` and
  `from_form` bodies for a representative struct.
- Host unit tests that exercise the generated `from_form` against sample bodies,
  including the error mappings.

See [08-testing.md](08-testing.md) for the full strategy.

## Open questions / risks

- Edition 2024 + `syn` 2: confirm attribute parsing ergonomics (`Meta`/`Expr`).
- Keeping emitted code `no_std`/no-alloc — the macro runs with `std`, but its
  output must not pull in `std` or the allocator.
- Deciding the canonical attribute namespace (`provision(...)`) and error text
  early, since it is part of the public contract.
