# Feature 7 — Customization contract

## Goal

Specify and document the attribute-driven theming surface so users can inject
their own CSS, JS, and HTML (logo, header, footer) and restyle every generated
element — all at compile time, with zero runtime cost. This document is the
public contract; implementation is shared with Features 1 and 2.

## Subtasks

- [M1] Specify the container-level attributes, each accepting a `&str`
  (typically `include_str!`):
  - `css` — injected into a `<style>` tag in `<head>`
  - `js` — injected into a `<script>` tag
  - `header` — HTML fragment rendered in the header slot (logo, title, banner)
  - `footer` — HTML fragment rendered in the footer slot
- [M1] Document the **CSS ID/class naming scheme** for every generated element so
  users can fully restyle without forking (the canonical table lives in
  [02-html-generation.md](02-html-generation.md) and is mirrored below).
- [M1] Per-field special-input attributes — `#[provision(secret)]` (renders
  `type="password"`) and `#[provision(input_type = "...")]` — so special cases
  are explicit macro opt-ins rather than field-name heuristics.
- [M2] Richer per-field override attributes (`id`, `class`, `label`,
  `placeholder`, `validate`) and a dedicated "kitchen-sink" customization example
  crate under `examples/`.

## Example (target API)

```rust
#[derive(Provision)]
#[provision(
    css = include_str!("assets/theme.css"),
    js = include_str!("assets/portal.js"),
    header = include_str!("assets/header.html"),
    footer = include_str!("assets/footer.html"),
)]
struct MyConfig {
    ssid: heapless::String<32>,
    #[provision(default = "")]
    password: heapless::String<64>,
    use_dhcp: bool,
}
```

## CSS naming scheme (mirror of Feature 2)

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

## Test setup

Covered by the Feature 2 HTML-assertion tests: injected fragments appear in the
correct slots, and the IDs/classes above are present and stable across builds.

## Open questions / risks

- Trust model: injected fragments are author-provided at compile time and
  inserted verbatim (not escaped). Make this explicit so users understand they
  control that content.
- Keeping the naming scheme stable is an API commitment — changing an id/class is
  a breaking change for users' stylesheets. Treat the table as versioned.
