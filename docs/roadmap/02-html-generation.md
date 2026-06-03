# Feature 2 — Compile-time HTML generation

## Goal

Build the complete HTML page string on the host at macro-expansion time so the
device serves a single `&'static str` with no runtime formatting or allocation.
Every generated element must carry stable, documented CSS IDs and classes, and
the page must expose injection slots for user-supplied CSS, JS, and HTML
fragments.

## Subtasks

- [M1] Page scaffold: `<!doctype html>`, `<head>` (charset, `meta viewport`,
  injected `<style>` and `<script>` slots), `<body>` containing a header slot, a
  `<form method="post">`, and a footer slot.
- [M1] Per-field rendering: a `<label>` + `<input>` pair with **auto-generated
  IDs and classes** derived from the field name, following a stable, documented
  scheme (see below).
- [M1] Input-type inference from the field type:
  - `bool` → `<input type="checkbox">`
  - integer types → `<input type="number">`
  - `heapless::String<N>` / `&str` → `<input type="text">`
  - fields named like `password`/`secret` (or `secret` attr in M2) → `type="password"`
- [M1] Render `#[provision(default = ...)]` values into `value="..."` (or
  `checked` for checkboxes).
- [M1] HTML-escape all static text; emit a submit button; produce the whole page
  by host-side string concatenation so the result is `const`.
- [M2] A polished, still-overridable default stylesheet; a success/confirmation
  page template; per-field custom labels/placeholders/ids/classes from the
  Feature 1 [M2] attributes.

## CSS naming scheme (the contract)

Stable hooks users can target without forking. Proposed defaults:

| Element | id | class |
|---|---|---|
| page wrapper | `provision-page` | `provision` |
| header slot | `provision-header` | `provision-header` |
| footer slot | `provision-footer` | `provision-footer` |
| form | `provision-form` | `provision-form` |
| field wrapper | `provision-field-<name>` | `provision-field` |
| label | `provision-label-<name>` | `provision-label` |
| input | `provision-input-<name>` | `provision-input` |
| submit button | `provision-submit` | `provision-submit` |

This table is the canonical reference shared with [07-customization.md](07-customization.md).

## Public surface / signatures

Internal to the macro crate (no public runtime API). A host-side builder, e.g.:

```rust
// pseudo-signature inside provisioner-macro
fn build_html(fields: &[FieldSpec], attrs: &ContainerAttrs) -> String;
```

The resulting `String` is emitted as a `&'static str` literal.

## Test setup

- Host unit tests asserting the generated HTML contains the required IDs,
  classes, and inferred input types for each field type.
- Tests asserting injected fragments (css/js/header/footer) appear in the right
  slots.
- A full-page snapshot test for a representative struct.

## Open questions / risks

- Escaping strategy for user-injected fragments: header/footer/css/js are
  trusted (compile-time, author-provided) and inserted verbatim; only
  macro-generated static text is escaped. Document this clearly.
- Field-name → id sanitization rules (collisions, non-identifier characters).
