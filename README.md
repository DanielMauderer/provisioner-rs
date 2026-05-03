# provisioner-rs

> **Status: early development.** No platform is implemented yet; see the [Supported platforms](#supported-platforms) table.

Annotate a plain Rust struct and get a fully functional WiFi configuration portal — no hand-written HTTP, no hand-written HTML.

## Vision

Configuring an embedded device over WiFi typically means writing a web server, building HTML forms, wiring up routes, and manually parsing form fields. `provisioner-rs` eliminates all of that. You describe what you want to configure, the library handles everything else.

Once provisioning is complete and the device reboots, the library has zero runtime cost. No background threads, no WiFi stack, no HTTP server. Your struct is just data.

## Planned API

Add `#[derive(Provision)]` to your config struct. The macro inspects your fields at compile time and generates a complete HTML configuration page and a form parser — no runtime allocation required.

On first boot (no stored credentials), call `Provisioner::run`. The device starts a WiFi access point, serves the configuration portal, and blocks until the user submits the form. The validated config is persisted to flash and the device reboots into normal operation.

```rust
use provisioner::Provision;

#[derive(Provision)]
struct MyConfig {
    ssid: heapless::String<32>,
    password: heapless::String<64>,
    device_name: heapless::String<32>,
    use_dhcp: bool,
}

// Blocks until the user submits the form.
// After this returns, WiFi is torn down — config is plain data.
let config: MyConfig = Provisioner::<MyConfig>::run(peripherals.WIFI, storage).await;
```

Any field type that implements `core::str::FromStr` is supported: fixed-capacity strings, integers, booleans, or your own types.

## Supported platforms

| Platform | Feature flag | Status |
|----------|-------------|--------|
| ESP32-C3 | `esp32c3`   | Planned (v1) |

Platform-specific code (WiFi, flash storage) is isolated behind feature flags. The HTML generation and form parsing logic are hardware-agnostic and can support additional platforms in future releases.

```toml
[dependencies]
provisioner = { version = "0.1", features = ["esp32c3"] }
```
