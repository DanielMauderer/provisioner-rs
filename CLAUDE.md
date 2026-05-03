# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Commands

```bash
cargo build                          # compile default-members (library crates only, host)
cargo build -F esp32c3               # compile with ESP32-C3 platform code
cargo test                           # run all tests
cargo test <name>                    # run a single test by name (substring match)
cargo clippy                         # lint
cargo fmt                            # format
```

`examples/basic` is excluded from `default-members` because it enables `features = ["esp32c3"]`, which requires an ESP32 target and cannot compile on the host.

## Project goal

`provisioner-rs` is a `no_std` Rust library that lets a user annotate a plain struct with a derive macro and get a fully functional WiFi captive-portal configuration flow with zero hand-written HTTP or HTML code. After the device is configured and reboots, the library imposes zero runtime cost — the struct is just data.

## Workspace layout

```
provisioner-rs/
├── Cargo.toml                          # workspace root
├── crates/
│   ├── provisioner/                    # main library crate, platform selected via features
│   │   └── src/
│   │       ├── lib.rs                  # crate root; feature-gates platform, re-exports macro
│   │       ├── error.rs                # ParseError (implemented)
│   │       ├── storage.rs              # Storage trait (implemented)
│   │       ├── form.rs                 # URL-encoded form body decoder (stub)
│   │       ├── http.rs                 # HTTP/1.1 request parser (stub)
│   │       └── platform/
│   │           ├── mod.rs              # feature-gated platform selection
│   │           └── esp32c3/
│   │               └── mod.rs          # WiFi + HTTP server + Storage impl (stub)
│   └── provisioner-macro/              # proc-macro crate (#[derive(Provision)])
│       └── src/
│           └── lib.rs                  # derive(Provision) implementation (stub)
└── examples/
    └── basic/                          # minimal usage example
        └── src/
            └── main.rs
```

## Crate responsibilities

### `provisioner` (main library)

Platform-agnostic modules compiled unconditionally:
- `error` — `ParseError` returned when form fields fail to parse. **Implemented.**
- `storage` — `Storage` trait (`load`/`store` on `&[u8]`). **Implemented.** The v1 impl lives in the esp32c3 platform module.
- `http` — minimal HTTP/1.1 request parser operating on `&[u8]`. **Stub.**
- `form` — URL-encoded form body decoder. **Stub.**

Platform modules compiled only when the matching feature is enabled:
- `platform::esp32c3` — behind `feature = "esp32c3"`. Planned to own: WiFi SoftAP setup/teardown (`esp-hal` + `esp-wifi` + `embassy-net`), raw TCP HTTP server loop, and a `Storage` impl via `sequential-storage` + `esp-storage` (raw flash partition — no ESP-IDF NVS). **Stub.**

Re-exports `provisioner-macro::Provision` so users only need one dependency.

### `provisioner-macro`

Proc-macro crate (must be its own crate — Rust requirement). Exposes `#[derive(Provision)]`.

Runs on the host at compile time (full `std` available). Planned to emit for each annotated struct:
- `const HTML: &'static str` — complete HTML page with a `<form>` containing one input per field
- `impl ProvisionConfig` with `from_form(body: &str) -> Result<Self, ParseError>` that URL-decodes the POST body and calls `.parse::<FieldType>()` per field

**Field contract:** every field type must implement `core::str::FromStr`. This covers `heapless::String<N>`, `bool`, integers, and any user-defined type.

**Currently a stub** — `derive_provision` returns an empty `TokenStream`.

## Planned user-facing API

```rust
use provisioner::Provision;

#[derive(Provision)]
struct MyConfig {
    ssid: heapless::String<32>,
    password: heapless::String<64>,
    use_dhcp: bool,
}

// Peripheral token grants ownership — provisioner handles all WiFi setup internally.
// Blocks until form is submitted. After return: no threads, no WiFi, no HTTP.
let config: MyConfig = Provisioner::<MyConfig>::run(peripherals.WIFI, storage).await;
```

`Provisioner` and the full `ProvisionConfig` trait are not yet implemented.

## Key design constraints

- **No WiFi type in the public API.** Each platform module owns WiFi init and teardown internally. The user passes a raw peripheral token, not an initialized driver. This is what makes the API platform-agnostic without needing a WiFi trait.
- **HTML generated at compile time.** The proc-macro builds the HTML string on the host; no runtime allocation or formatting is needed on-device.
- **Platform isolation via features.** `esp-hal`, `esp-wifi`, and related crates are optional dependencies pulled in only by `features = ["esp32c3"]`. The portable modules (`error`, `storage`, `http`, `form`) never import platform crates.
- **Zero post-boot cost.** Once `Provisioner::run` returns, the caller owns a plain struct with no background tasks, no statics, and no driver references.
- **`no_std` throughout.** Use `heapless` for fixed-capacity collections. No `alloc` unless unavoidable.
- **Storage is a trait.** The v1 impl uses `sequential-storage` + `esp-storage`. The trait boundary allows future crates to provide alternative backends.
