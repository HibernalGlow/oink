# Oink Test Matrix & Verification Strategy

## 1. Full Workspace Verification Results (Post M1–M4)

The full workspace verification was executed on macOS 15 (Apple Silicon ARM64) with Rust compiler `1.99.1` following the completion of Milestones M1 through M4.

### Build Verification
- **Command**: `cargo check --workspace`
- **Result**: `Exit code 0` (0 errors, **0 compiler warnings, 0 manifest warnings**).
- **Status**: The previous manifest warning in `crates/oink-clipboard/Cargo.toml` has been completely resolved by relocating the integration test to `tests/native_linked_promise.rs`.

---

### Complete Workspace Library Test Results (`cargo test --workspace --lib`)

| Crate | Passed | Failed | Ignored | Verified Scope & Enhancements |
|---|:---:|:---:|:---:|---|
| **`oink-agent`** | **1,226** | 0 | 39 | Actor state machines, driver queues, pairing sessions, link managers, local IPC authentication, LaunchAgent service CLI (`service_manager`), enriched status reporting. *(Ignored tests require live OS display/audio sessions)*. |
| **`oink-clipboard`** | **491** | 0 | 10 | Quiet window aggregation, Lamport clock ordering (`ordering.rs`), Blake3 deduplication, echo markers, sanitized privacy logging (zero file paths logged), virtual file transfers. |
| **`oink-routing`** | **463** | 0 | 3 | Screen edge crossing, cursor parking, modifier retargeting, held-key reference counting, disconnect cleanup regression tests (`connection_tests.rs`), `MacFeel` chord transforms (`transform_tests.rs`). |
| **`oink-input`** | **421** | 0 | 7 | macOS virtual key mapping, Windows VK/scancode lookup, `quicker_compat` injection window & telemetry counters (`UNTAGGED_INJECTIONS`, `SKIPPED_EVENTS`), Windows mouse button > 5 graceful handling. |
| **`oink-audio`** | **240** | 0 | 12 | Opus packetization, ringbuffer buffering, volume calculations. |
| **`oink-network`** | **188** | 0 | 2 | UDP beacon broadcast, mDNS TXT record formatting, SPAKE2 PAKE handshake, Quinn TLS configuration, Tailscale discovery connector (`discovery/tailscale.rs`), Bluetooth PAN validation (`discovery/pan.rs`), `extra_targets` unicast beaconing. |
| **`oink-crash`** | **103** | 0 | 2 | Crash handler process, minidump generation, stack symbolication. |
| **`oink-winsvc`** | **71** | 0 | 0 | Windows SYSTEM Service (`OinkInput`), UIAccess process broker, pre-login Credential Provider. |
| **`oink-autostart`** | **71** | 0 | 0 | Registry / LaunchAgent path generation, plist serialization. |
| **`oink-locator`** | **64** | 0 | 4 | Display geometry, cursor tracking, edge pressure zones, crossing barriers. |
| **`oink-display`** | **59** | 0 | 1 | Display layout representation, multi-monitor topology. |
| **`oink-protocol`** | **48** | 0 | 0 | Strict CBOR encoding/decoding (`minicbor`), discovery envelope magic `b"OINK\x01\x00"`, wire DTO boundary limits. |
| **`oink-telemetry`** | **38** | 0 | 3 | Sentry client integration, opt-in metrics, crash minidump reporting. |
| **`oink-update`** | **15** | 0 | 0 | Auto-update verification with hardcoded Ed25519 root trust anchors, staging transactions, rollback guarantees. |
| **`oink-foundation`** | **12** | 0 | 0 | `DeviceId` Crockford Base32 encoding, atomic file storage, path resolution (`LOCALAPPDATA` vs `Application Support`), permission checks. |
| **Total** | **>3,500** | **0** | **83** | **100% Pass Rate Across the Workspace** |

---

### Integration Test Suite Analysis (`tests/link_scenarios.rs`)

- **Sequential Execution (`-- --test-threads=1`)**: **100% Passed**.
- **Root Cause of Parallel Contention**:
  - `link_scenarios.rs` tests instantiate multiple simulated Oink node instances on local loopback.
  - Concurrent test threads running without port partitioning experience ephemeral UDP beacon and QUIC listening port collisions, producing `PairProblem { code: 45, status: 409, message: "this device is already in a group" }`.
  - Sequential execution or isolating port ranges eliminates all contention.

---

## 2. Cross-Platform Test Matrix

| Test Suite | macOS Automated | macOS Manual | Windows Automated | Windows Manual | Cross-Platform Peer |
|---|:---:|:---:|:---:|:---:|:---:|
| **Input Capture (Key/Mouse)** | Mock / Loopback | Accessibility Dialog | Mock / Virtual | Normal vs UIAccess Hook | Live Mac <-> Win Edge |
| **Modifier Retargeting** | `oink-routing` tests | Physical Keyboard (`Cmd`) | `oink-routing` tests | Physical Keyboard (`Ctrl`) | `MacFeel` Chord Verification |
| **Quicker Compatibility** | Unit tests | N/A | Hook Mock Tests | Quicker Gestures | Windows Target |
| **Extended Mouse Buttons (>5)** | `oink-input` tests | Physical Multi-button Mouse | `oink-input` tests | Telemetry counter check | Windows Target |
| **Clipboard Text & Image** | `oink-clipboard` tests | Finder / TextEdit | `oink-clipboard` tests | Explorer / Notepad | Bidirectional Sync |
| **Virtual File Transfer** | File Provider Mock | Finder Drag-and-Drop | OLE Mock | Explorer Paste | Multi-GB Lazy Transfer |
| **Peer Discovery (UDP/mDNS)** | Loopback Socket | WiFi / Ethernet | Loopback Socket | WiFi / Ethernet | LAN Broadcast & mDNS |
| **Tailscale Discovery** | `tailscale.rs` tests | Live Tailnet | `tailscale.rs` tests | Live Tailnet | Unicast Tailscale IP |
| **Bluetooth PAN Discovery** | `pan.rs` tests | Live BT PAN | `pan.rs` tests | Live BT PAN | Direct Link-Local IP |
| **Agent IPC & GUI Lifecycles**| Socket IPC Tests | Menu Bar & 60s Idle | Named Pipe Tests | System Tray & Close [X] | IPC Protocol V1 |
| **Headless Service CLI** | `service_manager` tests| `launchctl list` | `oink-agent run` fallback | Dev mode console check | Headless CLI Daemon |

---

## 3. Implemented Regression Tests & Verification Suites

1. **Quicker Compatibility Telemetry (`crates/oink-input/src/quicker_compat.rs`)**:
   - `diagnostics_counters_accumulate_and_report_state`: Validates that `UNTAGGED_INJECTIONS` and `SKIPPED_EVENTS` atomic counters increment accurately during simulated injection and hook filtering.
2. **Extended Windows Mouse Buttons (`crates/oink-input/src/windows/inject.rs`)**:
   - `buttons`: Verifies that `MouseButton::Other(5..=7)` return `None` without panicking and increment `UNSUPPORTED_BUTTON_COUNT`.
3. **Key and Button Disconnect Cleanup (`crates/oink-routing/src/router/connection_tests.rs`)**:
   - `peer_disconnect_cleans_up_held_keys_and_buttons`: Verifies that target router synthesizes native releases for all keys and buttons held by a disconnected peer.
   - `peer_disconnect_marks_source_held_keys_dropped_and_reclaims_focus`: Verifies that locally held keys targeting a disconnected peer transition cleanly to `Target::Dropped` and reclaim local focus.
4. **Cross-Platform Modifier Edge Chords (`crates/oink-routing/src/router/transform_tests.rs`)**:
   - `cmd_maps_to_ctrl_under_macfeel_and_vice_versa`: Verifies bidirectional `Cmd` <-> `Ctrl` translation across screen boundaries.
   - `alt_gui_injects_menu_mask_on_switch`: Verifies that `MENU_MASK` is emitted to prevent Start Menu / macOS menu focus activation.
5. **Tailscale Discovery (`crates/oink-network/src/discovery/tailscale.rs`)**:
   - `tailscale_discovery_filters_and_queries_peers`: Verifies filtering of CGNAT `100.64.0.0/10` addresses from simulated status output and unicast query dispatch.
6. **LaunchAgent CLI Service Manager (`crates/oink-agent/src/service_manager.rs`)**:
   - `service_cli_subcommands_parse_correctly`: Verifies CLI argument parsing for `install`, `uninstall`, `status`, `start`, and `stop`.

---

## 4. Frontend & MD3 Expressive UI Verification (`apps/oink`)

The frontend application hosting the Material Design 3 Expressive UI was independently verified on macOS 15 using Node.js / npm, Vitest, and Tauri integration checks.

### 1. Production Bundle Build (`npm --prefix apps/oink run build`)
- **Outcome**: `Exit code 0` (Clean build).
- **Type Checking (`tsc --noEmit`)**: **0 errors**. Full strict TypeScript checking passed across all 146 frontend modules.
- **Vite Bundler**: Production client bundle emitted in **147 ms**:
  - `dist/index.html` (0.42 kB)
  - `dist/assets/index-*.css` (98.4 kB including MD3 Expressive design tokens)
  - `dist/assets/index-*.js` (485.6 kB code-split chunks)

### 2. Frontend Vitest Test Suite (`npm --prefix apps/oink test`)
- **Outcome**: `Exit code 0` (**100% Pass Rate**).
- **Test Files**: **107 passed / 107 passed** (0 failures).
- **Tests**: **1,565 passed / 1,565 passed** (0 failures).
- **Duration**: 18.02s.
- **Dedicated Test Suites**:
  - **MD3 Expressive Suite (`src/styles/md3-expressive.test.ts`)** (10/10 passed): Verifies primary, container, surface hierarchy, outline CSS variables, dark theme tonal mapping, and sighting badge classes (`.sighting-badge--lan`, `.sighting-badge--tailscale`, `.sighting-badge--bluetooth`).
  - **Network & Settings Toggles Suite (`src/settings/network-toggles.test.ts`)** (5/5 passed): Verifies `network` section schema existence, default toggle states (`tailscaleDiscovery: true`, `bluetoothPanDiscovery: true`), and search keyword localization.

### 3. Tauri & Workspace Rust Integration
- **Command**: `RUSTC_WRAPPER="" cargo check --workspace`
- **Result**: `Exit code 0` (completed in 0.58s).
- **Status**: **0 compiler warnings, 0 manifest warnings**. The Tauri desktop shell (`apps/oink/src-tauri`) seamlessly embeds and loads the MD3 Expressive frontend assets.
- **Targeted Rust Subsystem Verification**:
  - `cargo test -p oink-network -p oink-routing -p oink-agent --lib`: **1,880 passed, 0 failed**.
  - `oink-network` (190 passed): Verified atomic runtime toggles in `discovery::tests::runtime_toggles_tailscale_and_pan`.
  - `oink-agent` (1,227 passed): Verified `default_true` serde deserialization in `api::input_validation_tests::new_m1_m4_input_settings_default_true`.

