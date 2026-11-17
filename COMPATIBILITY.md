# Oink Platform Compatibility & Third-Party Integration Matrix

## 1. macOS vs Windows Platform Matrix

Oink bridges two fundamentally different desktop operating systems. The table below details how each subsystem is implemented across macOS and Windows, including all enhancements introduced in Milestones M1 through M4.

| Feature Area | macOS Implementation | Windows Implementation | Architectural Parity & Implemented Status |
|---|---|---|---|
| **Input Capture** | Quartz `CGEventTap` (`kCGSessionEventTap` / `kCGHIDEventTap`). Runs on AppKit main event loop. Synthesizes KeyDown/KeyUp from `FlagsChanged` bitmasks. Self-echo filtered via `EventSourceUserData == 0x4f494e4b`. | Win32 Low-Level Hooks (`WH_KEYBOARD_LL`, `WH_MOUSE_LL`). Low-level message pump. Converts VK + scancodes (`LLKHF_EXTENDED`) to HID usages. Filtered via `dwExtraInfo == INJECT_TAG`. | **Full Parity**. Both backends emit normalized `InputEvent` enum to `oink-routing`. |
| **Input Injection** | `CGEventPost` with `CGEventSourceUserData`. Modifiers injected via merged `CGEventFlags`. Auxiliary keys (brightness, media) via `sysevent::post_aux_key`. | Win32 `SendInput` using `INPUT_KEYBOARD` and `INPUT_MOUSE`. Keycodes translated to Virtual Keys and scancodes. `INJECT_TAG` passed in `dwExtraInfo`. | **Full Parity**; privilege boundaries managed via Windows UIAccess token. |
| **Modifier Mapping** | `MacFeel` setting maps macOS `Command` (`Cmd`) to Windows `Control` (`Ctrl`) when crossing to Windows. Apple Fn key (`KeyCode::APPLE_FN`) handled safely without retriggering Globe shortcuts on return. | Windows `Ctrl` maps to macOS `Cmd` under `MacFeel`. Windows key (`GUI`) maps to macOS `Option` or `Cmd` based on user preference. `MENU_MASK` injected for held Alt/Win to prevent Start/Menu popup. | **Verified**. Covered by bidirectional unit tests in `crates/oink-routing/src/router/transform_tests.rs`. |
| **Mouse Buttons** | Supports arbitrary mouse button numbers (Buttons 1–32) via Quartz event types. Forward/Back mapped to buttons 4 & 5. | Win32 `SendInput` natively supports 5 buttons (`Left`, `Right`, `Middle`, `XBUTTON1`, `XBUTTON2`). Buttons > 5 (`MouseButton::Other(5..=7)`) handled gracefully: returns `None`, logs debug, increments `UNSUPPORTED_BUTTON_COUNT`. | **Hardened in M1**. No panics on extended buttons. |
| **Cursor Parking / Lock** | `CGDisplayHideCursor` + warps cursor to display corner. Delta extracted via `warp_delta.rs`. | `ClipCursor` restricts pointer to 1x1 pixel parking rect; reports relative deltas from raw hook coordinates. | **Full Parity**. Local cursor invisible while remote focus is active. |
| **Clipboard Detection** | Dedicated watcher thread polling `NSPasteboard.generalPasteboard.changeCount` every 75 ms. Wrapped in `autoreleasepool` and gate lock. | Dedicated `WinClipboard::worker` thread running Win32 message loop with `AddClipboardFormatListener` / `WM_CLIPBOARDUPDATE`. | **Full Parity**. Both trigger `Input::LocalChange(count)` with quiet window aggregation. |
| **Clipboard Markers** | Private marker format written to pasteboard: `MARKER_MAC = "com.hachiwii.oink.internal"`. Detected instantly without content reading. | Registered clipboard format: `MARKER_WINDOWS = "OinkInternalMarker"`. Detected via Win32 `IsClipboardFormatAvailable`. | **Full Parity**. Markers stripped from inventory before remote broadcast (`without_marker`). |
| **Virtual File Transfers** | Implements macOS File Provider / lazy URL publishing. Target registers lazy items with NSFileCoordinator; contents streamed on-demand. | Implements OLE `IDataObject` virtual files with `CF_HDROP` / `FileGroupDescriptorW` + `FileContents` (`IStream`). Content pulled lazily on paste. | **Full Parity**. Lazy semantics preserved on both platforms. Zero path leakage in logs. |
| **mDNS / DNS-SD** | Native Apple `dns_sd.h` (`DNSServiceRegister`, `DNSServiceBrowse`) on dedicated thread `oink-dnssd`. Fully compliant with macOS 15+ Sequoia Local Network Privacy. | Pure-Rust `mdns-sd` (v0.21) on dedicated thread `oink-mdns`. Binds all non-loopback IPv4 interfaces via `ServiceInfo::enable_addr_auto()`. | **Full Parity**. Announcements include identical TXT records (`wire=1`, `id`, `gid`, `os`). |
| **Tailscale Discovery** | Queries `tailscale status --json`, filters `100.64.0.0/10` peers, executes unicast `Discovery::query`, and sets `extra_targets` for periodic beacons. | Same cross-platform implementation in `crates/oink-network/src/discovery/tailscale.rs`. Supports named pipe socket for Windows Tailscale LocalAPI. | **Implemented in M2**. Seamless discovery over Tailscale without LAN broadcast. |
| **Bluetooth PAN Discovery** | Validates `bnep0` and link-local `169.254.0.0/16` interfaces; calculates subnet broadcast and fallback unicast queries. | Validates Bluetooth Network NDIS adapters and `169.254.0.0/16` link-local interfaces. | **Implemented in M2**. Supports direct zero-configuration links. |
| **Process Topology** | Unified `.app` bundle: `oink-backend` runs as persistent `LSUIElement` (status item); spawns `Contents/Helpers/Oink.app` for Tauri UI on demand. | Three-tier architecture: `OinkInput` (SYSTEM Service) -> `oink.exe --service` (UIAccess Backend) -> `oink.exe` (User Tray + WebView2). Dev mode fallback supported. | **M4 Added Dev Fallback**. `oink-agent run` on Windows falls back to standard user mode if service is absent. |
| **Headless Service CLI** | `oink-agent service install/uninstall/status/start/stop` generates and manages LaunchAgent plist in interactive Aqua session. | Managed via Windows Service Manager (`sc.exe` / `oink-winsvc`) or `oink-agent run` in dev mode. | **Implemented in M4**. Native service management on both platforms. |
| **Autostart** | Modern macOS ServiceManagement API (`SMAppService.mainAppService()`) or user LaunchAgent (`com.hachiwii.oink-agent.plist`). | Standard `HKCU\Software\Microsoft\Windows\CurrentVersion\Run` or Task Scheduler task. | **Full Parity**. Platform-native autostart supported cleanly on both. |
| **Audio Streaming** | CoreAudio HAL / Aggregate Devices via `objc2-core-audio`. Bidirectional Opus codec streaming. | WASAPI loopback capture and playback with VB-CABLE virtual driver integration. | **Full Parity** for audio and microphone forwarding. |

---

## 2. Third-Party Software & Tooling Interoperability

### 1. Quicker Automation Compatibility (Windows)
- **Problem**: Quicker (v1.45.5+) on Windows is an automation and gesture tool. By design, Quicker ignores mouse button events whose `dwExtraInfo` is non-zero (it does not check `LLMHF_INJECTED`). Standard Oink injections tag `dwExtraInfo = 0x4f494e4b`, preventing Quicker from recognizing injected mouse gestures.
- **Implemented Architecture** (`crates/oink-input/src/quicker_compat.rs`):
  - **Injection Path**: When `quicker_compat` is enabled, Oink clears `dwExtraInfo = 0` during mouse button injection and arms a 1-second injection window: `MOUSE.arm(Instant::now())`. Increments `UNTAGGED_INJECTIONS` atomic telemetry counter.
  - **Hook Filter Path**: The low-level mouse hook checks `quicker_compat::skip(window_active, injected, own, release)`. If within the 1-second window, an event marked `LLMHF_INJECTED` with `dwExtraInfo == 0` that is **not** a button release is passed directly to `CallNextHookEx` without Oink processing. Increments `SKIPPED_EVENTS` atomic telemetry counter.
  - **Button Release Guarantees**: Release events (`WM_L/R/M/XBUTTONUP`) are **never** skipped. This guarantees that held-key and held-button tracking ledgers never get corrupted or stuck.
  - **Telemetry API**: `QuickerDiagnostics { enabled, window_active, untagged_injections, skipped_events }` is exposed on `InputBackend` and via `quicker_diagnostics()` for UI and monitoring.

### 2. Tailscale & Mesh VPN Networks
- **Network Environment**:
  - Tailscale operates over a virtual network adapter (`utunX` on macOS, Tailscale virtual adapter on Windows) using the CGNAT IPv4 address range `100.64.0.0/10`.
  - **Constraint**: Tailscale virtual adapters do **not** support subnet broadcast or multicast. Consequently, Oink's default UDP broadcast beacons (`255.255.255.255:47471`) and mDNS packets do not traverse the tailnet.
- **Implemented Solution (Milestone M2)**:
  - Added `crates/oink-network/src/discovery/tailscale.rs`: `TailscaleDiscovery` actively parses online peers via `tailscale status --json` (with socket fallback for LocalAPI).
  - Automatically queries peer endpoints via `Discovery::query` and registers their addresses into `Discovery::extra_targets`.
  - Periodic unicast UDP beacons (`Packet::Beacon`) are sent directly to Tailscale peer IP addresses.
  - Emits sightings with `Source::Tailscale` (mapped into API sightings in `crates/oink-agent/src/config_module/pairing_api.rs`).
  - Quinn QUIC transport connects directly over Tailscale UDP; mutual TLS 1.3 cryptographically authenticates each peer's Ed25519 `DeviceId`.

### 3. Bluetooth PAN (Personal Area Network) & Direct Links
- **Network Environment**:
  - Bluetooth PAN (BNEP) creates an Ethernet-like point-to-point IP link between devices (`bnep0` on macOS, Bluetooth Network Connection on Windows).
  - Devices obtain IP addresses via Internet Sharing or fallback to link-local addresses (`169.254.0.0/16`).
- **Implemented Validation (Milestone M2)**:
  - Added `crates/oink-network/src/discovery/pan.rs`: validates and monitors Bluetooth PAN and link-local interfaces.
  - Subnet broadcasts and unicast probes guarantee pairing and session establishment even on network stacks where broadcast is dropped.

### 4. macOS 15+ Sequoia Local Network Privacy
- Starting in macOS 15, applications accessing the local network trigger an OS permission prompt.
- Oink links against Apple's system `dns_sd.h` via `libSystem` in `crates/oink-network/src/discovery/dnssd.rs`. By using Apple's official system daemon (`mDNSResponder`), Oink avoids false-positive firewall alerts and complies with macOS privacy requirements.

### 5. Windows User Interface Privilege Isolation (UIPI) & Dev Mode Fallback
- On Windows, a Medium integrity application cannot inject input into High or SYSTEM integrity applications (e.g. Task Manager, elevated terminals, UAC dialogs).
- **Production Mode**: `oink-service.exe` (SYSTEM) launches `oink.exe --service` into the active user session with a duplicated token holding `UIAccess = TRUE`.
- **Developer Mode Fallback (M4)**: If `OinkInput` service is not installed (e.g. during local cargo development), `oink-agent run` automatically detects this and falls back gracefully to `run_direct_service()` in standard user mode with a clear console notification, allowing development without elevated service registration.

---

## 3. Privacy & Logging Audit: Completed Remediations

All five privacy leak locations identified in M0 have been remediated in Milestone M1, verified via `cargo check` and automated tests:

### 1. macOS File Provider Logging (`crates/oink-clipboard/src/macos/files.rs`)

#### Remediated: Line 64
- **Previous**:
  ```rust
  tracing::debug!(path = %root.display(), "读不到被复制的条目：{e}");
  ```
- **Remediated Code**:
  ```rust
  tracing::debug!(error = %e, "读不到被复制的条目（已脱敏）");
  ```
- **Verification**: Zero local file paths emitted.

#### Remediated: Line 164
- **Previous**:
  ```rust
  tracing::debug!(path = %dir.display(), "读不了文件夹：{e}");
  ```
- **Remediated Code**:
  ```rust
  tracing::debug!(error = %e, "读不了文件夹（已脱敏）");
  ```
- **Verification**: Zero local directory paths emitted.

---

### 2. Windows File Provider Logging (`crates/oink-clipboard/src/windows/files.rs`)

#### Remediated: Lines 284 & 330
- **Previous**:
  ```rust
  tracing::warn!("读取 {} 失败：{e}", top.display());
  tracing::warn!("列出 {} 失败：{e}", dir.display());
  ```
- **Remediated Code**:
  ```rust
  tracing::warn!(error = %e, "读取顶层条目失败（已脱敏）");
  tracing::warn!(error = %e, "列出子目录失败（已脱敏）");
  ```
- **Verification**: Full Windows filesystem paths stripped from warning logs.

#### Remediated: Line 389
- **Previous**:
  ```rust
  tracing::warn!("文件 {} 没有本机路径，不能写成 CF_HDROP，跳过", e.path);
  ```
- **Remediated Code**:
  ```rust
  tracing::warn!("文件没有本机路径，不能写成 CF_HDROP，跳过");
  ```
- **Verification**: Relative paths and filenames removed from logs.

---

### 3. Windows Virtual File OLE Logging (`crates/oink-clipboard/src/windows/vfile.rs`)

#### Remediated: Line 1537
- **Previous**:
  ```rust
  tracing::trace!("Stat({}) {}", grfstatflag.0, self.name);
  ```
- **Remediated Code**:
  ```rust
  tracing::trace!(flag = grfstatflag.0, "Stat invoked on virtual file");
  ```
- **Verification**: Virtual file names stripped from OLE `IStream::Stat` trace logs.
