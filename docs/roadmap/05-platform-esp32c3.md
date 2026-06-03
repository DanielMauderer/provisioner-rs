# Feature 5 — ESP32-C3 platform + orchestration

## Goal

Own WiFi setup and teardown internally and run the portal loop, exposing the
single public entry point `Provisioner::<T>::run(peripheral, storage).await`.
The user passes a raw peripheral token (no initialized WiFi driver) — this is
what keeps the public API platform-agnostic without a WiFi trait. After `run`
returns, there must be zero residual tasks, statics, or driver references.

Lives behind `feature = "esp32c3"` in `platform::esp32c3`, using `esp-hal`,
`esp-radio`, `embassy-net`, `embassy-executor`, `sequential-storage`, and
`esp-storage`.

## Subtasks

- [M1] Initialize `esp-hal` + `esp-radio` WiFi in SoftAP mode from the passed
  peripheral token; bring up the `embassy-net` stack.
- [M1] Minimal DHCP server / address handout so connecting clients get an IP.
- [M1] Captive-portal DNS responder that resolves all queries to the device IP
  (so probe requests reach our HTTP server).
- [M1] TCP accept loop on port 80: read bytes → `http::parse_request` → on `GET`
  serve `T::HTML`, on `POST` decode the body via `form.rs` and call
  `T::from_form`.
- [M1] On a valid submit: persist via the `Storage` impl (Feature 6), tear down
  WiFi / network stack / spawned tasks, and return the populated `T`. Verify no
  residual background work remains.
- [M1] Boot-decision helper: if storage already holds a valid config, skip the
  portal entirely and return immediately — preserving the zero post-boot cost
  guarantee.
- [M2] Error/retry UX (re-render the form with an inline error on parse
  failure), client/idle timeouts, multi-client handling, optional WPA2 on the
  AP, and a re-provision trigger (e.g. a button/flag to force the portal).

## Public surface / signatures

```rust
// platform::esp32c3 (sketch)
pub struct Provisioner<T>(core::marker::PhantomData<T>);

impl<T: ProvisionConfig> Provisioner<T> {
    pub async fn run(wifi: WIFI, storage: impl Storage) -> T;
}
```

Matches the planned user-facing API in `CLAUDE.md`:

```rust
let config: MyConfig = Provisioner::<MyConfig>::run(peripherals.WIFI, storage).await;
```

## Test setup

- Host build (default members) must stay green; the platform module is
  feature-gated so it never breaks host compilation.
- The existing ESP32-C3 cross-build CI job (`--features esp32c3`) must stay
  green.
- End-to-end gate: document a QEMU/Wokwi simulation or an on-hardware manual
  smoke test (connect phone → form appears → submit → value persisted →
  device returns struct). This is the Milestone 1 acceptance test.

See [08-testing.md](08-testing.md).

## Open questions / risks

- DHCP + DNS responder size: keep them minimal and self-contained to avoid
  heavy dependencies.
- Clean teardown of `embassy` tasks and the radio is the trickiest part of the
  "zero post-boot cost" guarantee — needs careful ownership design.
- Whether `run` spawns an executor itself or assumes one is running — decide and
  document the contract.
