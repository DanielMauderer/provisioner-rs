# provisioner-rs Roadmap

This directory is the planning home for `provisioner-rs`. It enumerates every
feature required for the end product, breaks each into small subtasks, and
divides the work into two milestones.

## Vision (recap)

Annotate a plain struct with `#[derive(Provision)]` and get a complete WiFi
captive-portal configuration flow — HTML form, form parsing, HTTP server, WiFi
SoftAP, and flash storage — with **zero runtime cost after provisioning** and
**HTML generated entirely at compile time**. Once `Provisioner::run` returns,
the caller owns a plain struct: no background tasks, no statics, no driver
references.

## Locked design decisions

1. **Customization is compile-time, via macro attributes.** Custom CSS, JS, and
   HTML injection points (logo, header, footer) are supplied through derive
   helper attributes — typically with `include_str!`. No runtime builder. This
   preserves the "HTML at compile time / zero runtime cost" guarantee.
2. **Every generated element carries stable CSS IDs and classes** so users can
   fully restyle the portal without forking the crate.
3. **Milestone 1 is a full end-to-end vertical slice on real ESP32-C3
   hardware** — not just host-testable pieces.
4. **Field configuration is minimal for Milestone 1**: input type inferred from
   the field *type*, plus default values and auto-generated IDs/classes. Special
   input cases (e.g. password) are opt-in via macro attributes
   (`#[provision(secret)]` / `input_type = "..."`), never inferred from the field
   name. Richer per-field options come in the First Full Release.

## Current state

Implemented today: `ParseError` (`error.rs`) and the `Storage` trait
(`storage.rs`). Stubbed/empty: `form.rs`, `http.rs`,
`platform::esp32c3`, and the `provisioner-macro` derive (a no-op). There are no
tests yet. CI already runs fmt, clippy, host tests, and an ESP32-C3 cross-build.

## Milestones

### Milestone 1 — Minimal Requirements (MVP)

A user can flash `examples/basic`, the device boots into a SoftAP, a phone
connects and is redirected to a styled form, submitting it persists the config
to flash, and `Provisioner::run` returns the populated struct with the radio
torn down. Field types are limited to the predefined set; customization is
limited to global CSS/JS plus header/logo injection.

### Milestone 2 — First Full Release (v1.0)

Rich per-field customization, broader field-type support and validation, a
polished default theme, hardened HTTP/WiFi edge cases, success/redirect pages,
full docs and a customization example, and a complete automated test suite
(including macro expansion snapshots and compile-fail tests).

## Feature documents

| # | Feature | Doc |
|---|---------|-----|
| 1 | `#[derive(Provision)]` macro | [01-derive-macro.md](01-derive-macro.md) |
| 2 | Compile-time HTML generation | [02-html-generation.md](02-html-generation.md) |
| 3 | Form decoder (`form.rs`) | [03-form-parser.md](03-form-parser.md) |
| 4 | HTTP/1.1 server primitives (`http.rs`) | [04-http-server.md](04-http-server.md) |
| 5 | ESP32-C3 platform + orchestration | [05-platform-esp32c3.md](05-platform-esp32c3.md) |
| 6 | Storage format & impl | [06-storage.md](06-storage.md) |
| 7 | Customization contract | [07-customization.md](07-customization.md) |
| 8 | Testing & CI | [08-testing.md](08-testing.md) |

## Feature → milestone matrix

| Feature | M1 (Minimal) | M2 (Full Release) |
|---|---|---|
| Derive macro | parse + codegen, container attrs, `default`/`secret`/`input_type` | rich field attrs, validation, generics |
| HTML generation | scaffold, auto IDs/classes, type inference | default theme, success page, per-field |
| Form decoder | decode + percent/`+`, errors | edge cases, limits |
| HTTP server | parse + responses + captive probes | robustness, chunking, keep-alive |
| ESP32-C3 platform | SoftAP+DHCP+DNS+loop+teardown+boot-skip | retry UX, timeouts, WPA2 |
| Storage | versioned format + esp impl | migration, corruption |
| Customization | css/js/header/footer + naming scheme | per-field overrides + example |
| Testing | unit + trybuild + expansion + html asserts | e2e sim, coverage, examples |

## Document template

Each feature doc follows the same structure:

- **Goal** — one paragraph.
- **Subtasks** — a checklist; each item is small enough to be a single PR and is
  tagged `[M1]` (Minimal Requirements) or `[M2]` (First Full Release).
- **Public surface / signatures** — a sketch of the API.
- **Test setup** — what to test and with which tool.
- **Open questions / risks**.
