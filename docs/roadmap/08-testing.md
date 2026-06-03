# Feature 8 — Testing & CI

## Goal

A test strategy covering every component, with particular emphasis on the
proc-macro (which is the easiest place for regressions to hide). Most of the
suite runs on the host in CI; only the final end-to-end gate needs hardware or
simulation.

## Where tests live

- `provisioner-macro` tests go in the macro crate:
  `crates/provisioner-macro/tests/` (required for `trybuild`/`macrotest`).
- Portable-module tests (`form.rs`, `http.rs`, storage round-trip) are
  `#[cfg(test)]` inline in the `provisioner` crate.
- Add `trybuild`, `macrotest` (or rely on `cargo expand` snapshots), and an
  in-memory `Storage` mock as `dev-dependencies`.

## Subtasks

- [M1] Host unit tests for `form.rs` (decode/percent/`+`/errors), `http.rs`
  (request parse + response build + probe endpoints), and storage round-trip via
  an in-memory `Storage` mock.
- [M1] Macro `trybuild` setup: `tests/compile_fail/` with `.rs` + `.stderr`
  pairs for non-struct input, unsupported field type, and malformed/unknown
  attribute. A `tests/pass/` set for valid derives that must compile.
- [M1] Macro expansion / snapshot tests (`macrotest` or `cargo expand`)
  verifying the generated `HTML` const and `from_form` body for a
  representative struct.
- [M1] HTML-generation assertion tests: required IDs, classes, and inferred input
  types are present; injected css/js/header/footer fragments land in the right
  slots.
- [M1] Keep the ESP32-C3 cross-build CI job green; add `--features esp32c3`
  `cargo check` coverage where it can run without a linker step.
- [M2] End-to-end simulation (QEMU/Wokwi) or a documented on-hardware smoke-test
  checklist as the Milestone 1 acceptance gate; coverage reporting; and an
  expanded `examples/` set including the customization example.

## CI integration

The existing pipeline already runs `fmt`, `clippy`, `cargo test`, and an
esp32c3 cross-build. Extend it so:

- new host unit + macro tests run under the existing `test` job,
- `trybuild`/expansion tests are part of `cargo test` (they are ordinary
  `#[test]`s),
- the esp32c3 job continues to gate platform code.

## End-to-end acceptance (Milestone 1)

Manual or simulated, documented as a checklist:

1. Flash `examples/basic` to an ESP32-C3.
2. Device boots into a SoftAP; a phone connecting is redirected to the form.
3. The form is styled and shows the expected fields/IDs.
4. Submitting persists the config to flash.
5. `Provisioner::run` returns the populated struct; radio/tasks are torn down.
6. On the next boot, a stored valid config skips the portal.

## Open questions / risks

- `trybuild` `.stderr` files are sensitive to compiler version; pin or
  normalize, and document how to regenerate (`TRYBUILD=overwrite`).
- Simulation fidelity: Wokwi/QEMU may not fully emulate the WiFi radio, so the
  true end-to-end gate may need real hardware. Decide and document.
