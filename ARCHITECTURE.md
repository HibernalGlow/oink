# Oink System Architecture

## 1. System Overview & Design Philosophy

Oink is a high-performance, secure cross-device input sharing and clipboard synchronization system designed primarily for seamless interaction between macOS and Windows. 

### Core Design Principles
1. **Asymmetric Separation of Concerns**: Complete separation between the persistent background daemon (`oink-agent` / `oink-backend`) and the transient user interface (`oink-ui` via Tauri). The background agent must never crash or terminate when windows close, WebViews crash, or the user logs out.
2. **Zero-Trust Peer Security**: Devices communicate over QUIC (via Quinn) encrypted with mutual TLS 1.3 (`rustls`). Peer identity is cryptographically tied to raw Ed25519 public keys (`DeviceId`). Discovery and pairing employ SPAKE2 Password-Authenticated Key Exchange (PAKE), ensuring zero exposure of unencrypted metadata or private keys over local networks.
3. **Low-Latency Direct Path**: Input events bypass high-overhead serialization frameworks; raw HID usage codes and mouse deltas are streamed over QUIC streams with minimum framing overhead.
4. **Resilient Clipboard Semantics**: Distributed clipboard synchronization utilizes Lamport logical clocks with deterministic `DeviceId` tie-breaking to avoid distributed split-brain or infinite echo loops. Virtual placeholders ensure that multi-gigabyte files or directories are transferred strictly on-demand.

---

## 2. Workspace Crate Dependency Graph

Oink is organized as a Cargo workspace with 17 crates adhering to strict layering constraints.

```mermaid
graph TD
    subgraph Core Primitives
        FOUNDATION["oink-foundation<br/>(Paths, ID, Storage, Logging)"]
        PROTOCOL["oink-protocol<br/>(CBOR DTOs, Wire Framing)"]
    end

    subgraph Hardware & OS Abstractions
        INPUT["oink-input<br/>(CGEventTap / WinHook / SendInput)"]
        DISPLAY["oink-display<br/>(Displays, Coordinates)"]
        LOCATOR["oink-locator<br/>(Cursor Tracking, Edges)"]
        AUDIO["oink-audio<br/>(CoreAudio / WASAPI / Opus)"]
        AUTOSTART["oink-autostart<br/>(SMAppService / Registry / TaskScheduler)"]
        WINSVC["oink-winsvc<br/>(Windows Service & UIAccess)"]
    end

    subgraph Subsystems
        ROUTING["oink-routing<br/>(Screen Topology, Key Transformations)"]
        CLIPBOARD["oink-clipboard<br/>(Pasteboard Watcher, Blake3 Dedupe)"]
        NETWORK["oink-network<br/>(QUIC, mTLS, mDNS, Beacons, PAKE)"]
        UPDATE["oink-update<br/>(Ed25519 Catalog, Staging)"]
        CRASH["oink-crash<br/>(Minidump Symbolication)"]
        TELEMETRY["oink-telemetry<br/>(Sentry, Minidumps)"]
    end

    subgraph Applications & Daemons
        AGENT["oink-agent<br/>(Core Orchestrator Daemon)"]
        UI["oink-ui (apps/oink/src-tauri)<br/>(Tauri 2 Desktop Shell)"]
        TEST_SUPPORT["oink-test-support<br/>(Network & Mock Fixtures)"]
    end

    FOUNDATION --> INPUT
    FOUNDATION --> DISPLAY
    FOUNDATION --> LOCATOR
    FOUNDATION --> ROUTING
    FOUNDATION --> CLIPBOARD
    FOUNDATION --> NETWORK
    FOUNDATION --> PROTOCOL
    FOUNDATION --> UPDATE
    FOUNDATION --> AUDIO
    FOUNDATION --> AUTOSTART
    FOUNDATION --> WINSVC

    PROTOCOL --> NETWORK
    PROTOCOL --> AGENT

    INPUT --> ROUTING
    DISPLAY --> ROUTING
    LOCATOR --> ROUTING

    ROUTING --> AGENT
    CLIPBOARD --> AGENT
    NETWORK --> AGENT
    AUDIO --> AGENT
    UPDATE --> AGENT
    TELEMETRY --> AGENT
    CRASH --> AGENT
    WINSVC --> AGENT

    AGENT --> UI
    FOUNDATION --> UI
    AUTOSTART --> UI

    TEST_SUPPORT -.-> AGENT
```

### Workspace Crate Responsibilities

| Crate | Responsibility | Platform Dependencies |
|---|---|---|
| **`oink-foundation`** | Crockford Base32 `DeviceId`, secure atomic file storage (`0o600` on Unix, explicit owner DACL on Windows), process priority, path resolution (`LOCALAPPDATA` vs `Application Support`). | `windows-sys`, `libc`, `ed25519-dalek` |
| **`oink-protocol`** | Wire serialization, bounded framing, CBOR code generation (`minicbor`), capability admission, discovery protocol magic `b"OINK\x01\x00"`. | Architecture-neutral |
| **`oink-input`** | OS-level keyboard/mouse capture and injection. Translates native keycodes to/from neutral HID `KeyCode`. Quicker compatibility mode with atomic telemetry (`quicker_compat.rs`), Windows mouse button > 5 graceful handling. | macOS: `objc2-app-kit`, `objc2-core-graphics`. Windows: `windows` (Win32 Hook, `SendInput`) |
| **`oink-locator`** | Display geometry, cursor tracking, edge pressure zones, and crossing barriers. | Multi-platform |
| **`oink-display`** | Display layout representation, multi-monitor configuration, coordinate normalization. | Multi-platform |
| **`oink-routing`** | Central routing engine (`Router`). Screen switching, modifier retargeting, held-key reference counts, local click dispatch, menu masking, peer disconnect cleanup regression protection. | Multi-platform |
| **`oink-clipboard`** | OS clipboard monitoring (polling vs events), 75 ms quiet aggregation, Blake3 content hashing, internal echo markers, lazy virtual file transfers. Stripped of all local file path logging for privacy. | macOS: AppKit `NSPasteboard`. Windows: Win32 Clipboard, OLE `IDataObject` |
| **`oink-network`** | QUIC transport (`quinn`), mutual TLS with self-signed Ed25519 certificates, UDP beacon broadcasting, DNS-SD/mDNS, SPAKE2 PAKE handshake, Tailscale mesh discovery (`discovery/tailscale.rs`), Bluetooth PAN tracking (`discovery/pan.rs`), dynamic network change detection. | macOS: `dns_sd.h` (`libSystem`). Windows: `mdns-sd` (pure-Rust) |
| **`oink-audio`** | Bidirectional low-latency audio streaming using Opus codec. | macOS: CoreAudio HAL / Aggregate Devices. Windows: WASAPI / VB-CABLE |
| **`oink-autostart`** | System startup integration. | macOS: `SMAppService`. Windows: `HKCU\Run` & Task Scheduler |
| **`oink-winsvc`** | Windows SYSTEM Service (`OinkInput`), UIAccess process broker, pre-login Credential Provider. | Windows only |
| **`oink-update`** | Auto-update verification with hardcoded Ed25519 root trust anchors, transactional directory staging, and rollback support. | Multi-platform |
| **`oink-telemetry`** | Sentry client integration, opt-in metrics, crash minidump reporting. | Multi-platform |
| **`oink-crash`** | Crash handler process, minidump generation, stack symbolication. | Multi-platform |
| **`oink-agent`** | Orchestrator daemon. Spawns Tokio runtime, hosts actors for network, clipboard, and input, exposes local authenticated IPC (`ipc.json`), LaunchAgent CLI management (`service_manager.rs`), Windows unprivileged dev-mode fallback. | Multi-platform |
| **`apps/oink/src-tauri`** | Tauri 2.12 desktop shell, WebView2 / WKWebView presenter, system tray, UI IPC client. | Multi-platform |
| **`tests/support`** | Integration testing support, mock peers, network latency simulation. | Multi-platform |

---

## 3. Platform Process Topologies

Oink executes in distinct platform topologies tailored to the security and windowing models of macOS and Windows.

### macOS Process Topology
On macOS, the core agent runs as an `LSUIElement` background application hosting the status bar menu item. The Tauri UI helper runs on-demand as a separate application bundle.

```mermaid
graph TD
    subgraph macOS Bundle: Oink.app
        AGENT["CFBundleExecutable: oink-backend<br/>(LSUIElement = true)<br/>- Owns NSStatusItem (Menu Bar)<br/>- Runs Tokio Runtime & QUIC Node<br/>- CGEventTap Input Capture<br/>- Binds agent.sock -> ipc.json<br/>- Holds service.lock"]
        
        HELPER["Contents/Helpers/Oink.app<br/>(Executable: oink)<br/>- Tauri UI Process (WKWebView)<br/>- Connects to ipc.json<br/>- Binds ui.sock -> ui-ipc.json<br/>- 60s Idle Timer -> Clean Exit<br/>- Holds ui.lock"]
    end

    AGENT -->|On-Demand Launch via /usr/bin/open| HELPER
    HELPER -.->|Local IPC over Unix Domain Socket| AGENT
```

### Windows Process Topology
Windows enforces User Interface Privilege Isolation (UIPI). Injecting input into elevated applications (e.g. Task Manager, UAC prompts) requires a dedicated process running with `UIAccess = TRUE`.

```mermaid
graph TD
    subgraph Windows Session Architecture
        SVC["OinkInput (oink-service.exe)<br/>(NT AUTHORITY\\SYSTEM Service)<br/>- Session Launch Broker<br/>- Watches Session Changes"]
        
        AGENT["oink.exe --service<br/>(UIAccess Headless Process)<br/>- Runs in User Session with UIAccess=TRUE<br/>- Global WH_KEYBOARD_LL / WH_MOUSE_LL<br/>- Quinn QUIC Network & Routing<br/>- Binds Named Pipe -> ipc.json<br/>- Holds service.lock"]
        
        UI["oink.exe (Normal Desktop Process)<br/>(Medium Integrity)<br/>- Owns Win32 System Tray Icon<br/>- Hosts WebView2 UI Windows<br/>- Connects to Agent via ipc.json<br/>- Binds Named Pipe -> ui-ipc.json<br/>- Holds ui.lock"]
    end

    SVC -->|CreateProcessAsUser with UIAccess Token| AGENT
    UI -.->|Local IPC over Named Pipe| AGENT
```

---

## 4. The 4 Core Calling Chains

### Chain 1: Input Capture, Routing & Target Injection

Traces the complete path of mouse motion and key events from OS capture on the source machine to OS injection on the target machine.

```mermaid
sequenceDiagram
    autonumber
    participant SrcOS as Source OS (macOS/Win)
    participant Tap as oink-input (Tap/Hook)
    participant Router as oink-routing (Router)
    participant Driver as oink-agent (Driver/Wire)
    participant QUIC as QUIC Stream (Quinn)
    participant TgtRouter as Target Router
    participant Inject as Target oink-input (Inject)
    participant TgtOS as Target OS (Win/macOS)

    SrcOS->>Tap: Native Key / Pointer Event
    Tap->>Tap: Check Event Tag (Filter Self-Echo)
    Tap->>Router: on_local_event(InputEvent)
    
    alt Focus is Local (holder == me)
        Router->>Router: Check Screen Edge Boundary
        alt Crosses Edge
            Router->>Router: become_source(target_peer)
            Router->>Driver: Output::Send(InputMsg::Acquire)
            Router->>Tap: Hide/Park Local Cursor
        else Within Bounds
            Router-->>Tap: Disposition::Pass
            Tap-->>SrcOS: Deliver to Local Window
        end
    else Focus is Remote (holder != me)
        Router->>Router: Track Held State & Remap Modifiers
        Router->>Driver: Output::Send(InputMsg::Key / PointerMotion)
        Router-->>Tap: Disposition::Swallow
        Driver->>QUIC: hot::Message::InputKey (0x0202) / InputMotion (0x0201)
    end

    QUIC->>Driver: Receive Network Frame
    Driver->>TgtRouter: on_input(origin, InputMsg)
    TgtRouter->>TgtRouter: Apply KeyMap (Source->Target) & RefCounts
    TgtRouter->>Inject: Output::Inject(InjectEvent)
    Inject->>TgtOS: SendInput (Win) / CGEventPost (macOS) with INJECT_TAG
```

#### Detailed Execution Steps:
1. **OS Capture**:
   - macOS (`crates/oink-input/src/macos/tap.rs`): `CGEventTap` handles `kCGEventKeyDown`, `kCGEventKeyUp`, `kCGEventFlagsChanged`, and pointer moves. Modifiers are synthesized from `CGEventFlags` transitions. Events tagged with `MAGIC = 0x4f494e4b` are discarded.
   - Windows (`crates/oink-input/src/windows/hook.rs`): `WH_KEYBOARD_LL` and `WH_MOUSE_LL` intercept raw input. Events tagged with `dwExtraInfo == INJECT_TAG` are passed through.
2. **Routing Arbitration** (`crates/oink-routing/src/router.rs`):
   - Local pointer motion is evaluated against display boundaries (`layout.exit_through_edge`).
   - Crossing triggers `become_source(target)`: parks local cursor, retargets held modifiers (`Shift`, `Ctrl`, `Alt`, `GUI`), and sends `InputMsg::Acquire`.
   - Normal remote events are swallowed locally and emitted as `Output::Send`.
3. **Wire Encoding & Transport** (`crates/oink-agent/src/threads/input/driver.rs`):
   - Motion is coalesced if network backpressure occurs (`flush_motion_for`).
   - Keystrokes are encoded as CBOR frames (`0x0202`) and dispatched over the Quinn QUIC stream.
4. **Target Injection** (`crates/oink-input/src/macos/inject.rs` & `windows/inject.rs`):
   - Target translates neutral HID `KeyCode` back to native virtual keys or scancodes.
   - Modifiers update internal state bitmasks.
   - Native OS API injects event (`SendInput` with `INJECT_TAG` on Windows; `CGEventPost` with user data on macOS).

---

### Chain 2: Clipboard Discovery, Deduplication & Stream Transfer

Traces clipboard change observation, quiet window aggregation, cryptographic hashing, and lazy content transfer.

```mermaid
sequenceDiagram
    autonumber
    participant SrcOS as Source Pasteboard
    participant Watcher as oink-clipboard Watcher
    participant Task as Clipboard Task Actor
    participant Order as Ordering (Lamport Clock)
    participant Registry as Stream Registry
    participant Wire as QUIC Stream (LinkBulk)
    participant RemoteTask as Remote Clipboard Task
    participant TgtOS as Target Pasteboard

    SrcOS->>Watcher: Change Detected (ChangeCount / WM_CLIPBOARDUPDATE)
    Watcher->>Task: Input::LocalChange(count)
    Task->>Order: local_change(count, now)
    Note over Order: Slide Quiet Window (75 ms)<br/>Aggregates Multi-format App Writes
    
    Order->>Task: Quiet Window Expired -> Capture!
    Task->>SrcOS: Read Metadata & Generate Fingerprint
    Note over Task: Check Private Oink Marker<br/>Compute Blake3 Hash (Text / Image / File Meta)
    
    alt Content Unchanged (Duplicate Echo)
        Task->>Task: Suppress Broadcast
    else New Content Captured
        Task->>Order: capture() -> Increment Lamport Clock
        Task->>Registry: Register Data Providers (Inline / Readers)
        Task->>Wire: Send Message::ClipboardOffer (Reliable Stream)
    end

    Wire->>RemoteTask: Receive ClipboardOffer
    RemoteTask->>Order: receive(peer, offer.clock)
    Note over Order: Clock Reconciliation<br/>Check Ordering Fence (Clock + DeviceId)
    
    alt Local Copy In Flight or Offer Stale
        RemoteTask->>RemoteTask: Reject Offer
    else Offer Accepted
        RemoteTask->>TgtOS: write_native_cancellable(spec, guard)
        Note over TgtOS: Inject Virtual Descriptors / Placeholders<br/>Write Private Marker to Native Pasteboard
    end

    opt User Pastes on Target
        TgtOS->>RemoteTask: Request Content Bytes
        RemoteTask->>Wire: Pull Chunked Stream (LinkBulk)
        Wire->>Registry: Read Chunk Data
        Registry-->>Wire: Raw Bytes
        Wire-->>RemoteTask: Raw Bytes
        RemoteTask-->>TgtOS: Stream to Application
    end
```

#### Detailed Execution Steps:
1. **Change Detection**: macOS polls `changeCount` every 75 ms; Windows listens for `WM_CLIPBOARDUPDATE`.
2. **Quiet Period Aggregation**: A 75 ms sliding window (`SOURCE_QUIET_PERIOD`) coalesces successive clipboard formats written by copying applications (e.g. Rich Text followed by Plain Text).
3. **Capture & Deduplication**:
   - Inspects native clipboard for private markers (`MARKER_MAC` / `MARKER_WINDOWS`). If found, capture aborts (echo elimination).
   - Generates Blake3 fingerprint across text (normalized UTF-8), images (normalized RGBA8), and files (metadata only: name, size, mtime; file contents are never hashed).
4. **Ordering & Lamport Clocks**: Increments Lamport clock and persists state to `clipboard-clock.json`. Emits `Message::ClipboardOffer`.
5. **Target Application & Lazy Fetch**: Target receives offer, validates against Lamport tie-breaker, and injects placeholders into the native pasteboard. Heavy payloads and files are lazily streamed across low-priority bulk streams only upon user paste.

---

### Chain 3: Network Discovery, SPAKE2 PAKE & QUIC Links

Traces peer discovery over UDP beacons and mDNS, SPAKE2 mutual authentication, and TLS 1.3 QUIC connection establishment.

```mermaid
sequenceDiagram
    autonumber
    participant D1 as Device 1 (Initiator)
    participant Net1 as Network / Broadcast
    participant D2 as Device 2 (Responder)
    participant C0 as Quinn Control Stream (C0)

    Note over D1,D2: Phase 1: Peer Discovery
    D1->>Net1: UDP Beacon on 255.255.255.255:47471 (b"OINK\x01\x00" + Advert)
    D1->>Net1: mDNS Announce _oink._udp.local. (Apple dns_sd / mdns-sd)
    Net1-->>D2: Sighting(Advert, Addr, Source::Beacon/Mdns)
    
    Note over D1,D2: Phase 2: SPAKE2 PAKE Handshake
    D1->>D2: Connect QUIC (Self-signed TLS 1.3, ALPN b"oink-wire/1")
    D1->>C0: Message::ConfigPairRequest(PairRequest)
    D2->>C0: ConfigPairQueued -> User Confirms on UI
    D2->>C0: ConfigPairAccept
    D2->>C0: ConfigPairPake(PakeStart) [Responder Share]
    D1->>C0: ConfigPairPake(PakeReply) [Initiator Share + HMAC]
    D2->>C0: ConfigPairPake(PakeFinish) [Responder HMAC]
    Note over D1,D2: Verified Shared Secret! Exchange Admitted Group Keys
    
    Note over D1,D2: Phase 3: Established QUIC Session
    D1->>D2: Open N0 Stream (Input & Topology Control)
    D1->>D2: Open N1 Stream (Reliable Clipboard Offers)
    D1->>D2: Periodic Heartbeat (keepalive_interval = 1s, timeout = 3s)
```

#### Detailed Execution Steps:
1. **Discovery**:
   - UDP beacons broadcasted on port `47471` every 2s across all non-loopback IPv4 interfaces (`local_ipv4()`).
   - mDNS advertises `_oink._udp` with TXT records specifying `wire=1`, `id=<hex_device_id>`, `gid=<group_id>`.
   - Direct unicast query/reply (`DiscoveryQuery` / `DiscoveryReply`) verifies endpoints.
2. **SPAKE2 PAKE Pairing**:
   - Initiator displays a 6-digit code. Responder inputs code.
   - PAKE exchange establishes a cryptographic shared secret and verifies the transcript.
   - Nodes exchange Ed25519 `DeviceId`s and commit to `group-state.json`.
3. **QUIC Session Management**:
   - Quinn endpoint connects using mutual TLS 1.3. The SPKI in the self-signed certificate matches `DeviceId`.
   - Control stream `N0` supervises topologies, focus states, and heartbeats (1s ping, 3s timeout).

---

### Chain 4: Agent Daemon Lifecycle, Local Authenticated IPC & Tauri UI Presentation

Traces the process lifecycles, mutual exclusion locks, authenticated IPC, and graceful shutdown.

```mermaid
sequenceDiagram
    autonumber
    participant UI as Tauri UI Helper (oink)
    participant Pipe as Local IPC (Socket / Pipe)
    participant Host as Agent Daemon (oink-backend)
    participant OS as OS Kernel & Filesystem

    Note over Host: System Startup / App Launch
    Host->>OS: Acquire service.lock (flock / LockFileEx)
    Host->>OS: Generate 32-byte Random Token
    Host->>OS: Bind agent.sock (Unix) / Named Pipe (Win)
    Host->>OS: Publish ipc.json (Token + Port/Path) [Permissions: 0600]
    
    Note over UI: User Opens GUI Window
    UI->>OS: Acquire ui.lock
    UI->>OS: Read ipc.json -> Extract Token & Address
    UI->>Pipe: Connect & Authenticate ({"token": "...", "hello": true})
    Pipe-->>UI: ServiceInfo (Version, PID, Build)
    UI->>Pipe: Subscribe to Events ({"token": "...", "subscribe": true})
    
    Note over UI: User Closes Window [X]
    UI->>UI: Destroy WebviewWindow
    alt macOS
        UI->>UI: set_activation_policy(Accessory) -> Hide Dock
        UI->>UI: Arm 60-Second Idle Timer
        Note over UI: After 60s idle -> app.exit(0)
    else Windows
        Note over UI: UI Remains Resident in System Tray
    end
    
    Note over Host: Agent Remains Active Continuously!
    
    Note over UI,Host: User Selects "Quit Oink"
    UI->>Pipe: {"token": "...", "call": UiControl::request_stop}
    Host->>Host: Release Hooks, Restore Cursors, Flush Updates
    Host->>OS: Remove ipc.json & Release service.lock
    Host->>UI: UiEvent::Stopping
    UI->>OS: Remove ui-ipc.json & Release ui.lock
    UI->>OS: Exit Process
    Host->>OS: Exit Process
```

#### Detailed Execution Steps:
1. **Agent Startup**: Acquires `service.lock`, generates ephemeral 32-byte secret token, binds domain socket/named pipe, writes `ipc.json` with restricted permissions (`0600`).
2. **UI Connection**: Reads `ipc.json`, authenticates every JSON envelope via `token`, and opens event subscription stream.
3. **Closing Windows**: WebViews are destroyed to free memory (~15 MB footprint); on macOS, an idle timer terminates the UI after 60 seconds; on Windows, the process remains in the tray. The backend agent **never** shuts down when windows close.
4. **Shutdown**: "Quit Oink" triggers a clean drain sequence: hooks uninstalled, network sessions disconnected, locks and endpoint files unlinked.

---

## 5. State Files, Identity & Security Model

### Directory Resolution
Configured via `oink_foundation::paths`:
- **macOS**: `~/Library/Application Support/Oink` (Data), `~/Library/Caches/com.hachiwii.oink` (Cache), `~/Library/Logs/Oink` (Logs).
- **Windows**: `%LOCALAPPDATA%\Oink` (strictly excludes roaming `%APPDATA%`).
- **Environment Override**: `OINK_HOME` isolates all runtime paths for testing.

### Key State Files
- `device.key`: 32-byte raw Ed25519 seed. Formatted into 12-character Crockford Base32 `DeviceId` (`XXXX-XXXX-XXXX`). Unix mode `0600`; Windows owner-only DACL.
- `group-state.json`: Group membership list, admitted peer public keys, display arrangements, and pairing commitments.
- `clipboard-clock.json`: Lamport logical clock counter.
- `settings.json`: Local preferences (audio, routing edge resistance, Quicker compatibility, hotkeys).
- `service.lock` / `ui.lock`: OS-level mutual exclusion locks (`flock` on Unix; `LockFileEx` on Windows).
- `ipc.json` / `ui-ipc.json`: Ephemeral connection info containing socket path / pipe name and 32-byte hex token.

### Security Guarantees
- **No Unauthenticated IPC**: Every request must provide the exact matching 32-byte hex token.
- **Mutual TLS**: QUIC sessions require peer certificates signed by their claimed Ed25519 identity key.
- **Clipboard Isolation**: Pasteboard markers prevent recursive echo loops; Universal Clipboard promises are never read without explicit user paste.

---

## 6. MD3 Expressive Presentation & UI Architecture

The desktop presentation layer (`apps/oink`) leverages a comprehensive **Material Design 3 Expressive (MD3 Expressive)** design system (`apps/oink/src/styles/md3-expressive.css`) wrapped in Tauri 2.12 (WKWebView on macOS, WebView2 on Windows).

```mermaid
graph TD
    subgraph MD3 Expressive Design System
        TOKENS["Dynamic Color Tokens<br/>(:root & :root[data-theme='dark'])<br/>--md-primary / --md-surface / --md-outline"]
        SHAPES["Expressive Shapes & Radii<br/>--md-radius-pill (999px)<br/>--md-radius-squircle (28px 28px 12px 28px)<br/>.md-connected (Connected Groups)"]
        SPRINGS["Spatial Springs & Physics Motion<br/>--md-spatial-fast (350ms linear spring)<br/>--md-spatial-bounce (500ms bounce)<br/>--md-effects (180ms cubic-bezier)"]
        BADGES["Network Sighting Badges<br/>.sighting-badge--lan<br/>.sighting-badge--tailscale<br/>.sighting-badge--bluetooth"]
    end

    subgraph Oink Core Token Mapping
        MAPPING["--ok-bg -> --md-surface<br/>--ok-card -> --md-surface-low<br/>--ok-well -> --md-surface-container<br/>--ok-text -> --md-on-surface<br/>--ok-accent -> --md-primary<br/>--ok-line -> --md-outline-variant"]
    end

    subgraph UI Components & Shell
        APP["App Shell (app.css)<br/>Window Frames, Stage, Headers"]
        SETTINGS["Settings Panel (settings.css)<br/>Navigation Pills, Cards, Sliders"]
        DEVICES["Device Management (devices.css)<br/>Interactive Squircles & Peer Cards"]
        PAIRING["Pairing Flow (AddPanel.tsx)<br/>Multi-Network Badged Sightings"]
    end

    TOKENS --> MAPPING
    SHAPES --> APP & SETTINGS & DEVICES & PAIRING
    SPRINGS --> APP & SETTINGS & DEVICES & PAIRING
    BADGES --> PAIRING
    MAPPING --> APP & SETTINGS & DEVICES & PAIRING
```

### 1. Dynamic Color Roles & Surface Hierarchy
- **Light & Dark Mode Palettes**: Full MD3 Expressive tonal palettes defined in `:root` and `:root[data-theme='dark']`.
  - Roles: `--md-primary`, `--md-on-primary`, `--md-primary-container`, `--md-on-primary-container`, `--md-secondary-container`, `--md-tertiary-container`.
  - Surface Hierarchy: Five progressive elevation levels (`--md-surface`, `--md-surface-low`, `--md-surface-container`, `--md-surface-high`, `--md-surface-highest`).
  - Core Oink Variable Mapping: Seamlessly bridges legacy `--ok-*` variables to MD3 tokens (`--ok-bg`, `--ok-surface`, `--ok-card`, `--ok-well`, `--ok-text`, `--ok-accent`, `--ok-line`).

### 2. Expressive Shapes & Morphing Radii
- **Pill Buttons**: Full pill border-radius (`--md-radius-pill: 999px`) on primary action buttons, tabs, and navigation items.
- **Expressive Squircles**: Asymmetric organic corners (`--md-radius-squircle: 28px 28px 12px 28px` and `--md-radius-squircle-alt: 32px 8px 32px 32px`) applied to device cards, stage targets, and active focus indicators.
- **Connected Button Groups (`.md-connected`)**: Outer corners retain squircle curvature (`28px 8px 8px 28px` / `8px 28px 28px 8px`) while inner dividers are flattened, eliminating visual clutter.

### 3. Spatial Spring Motion & Physics
- Motion uses authentic spring physics curves via the modern CSS `linear()` function rather than artificial cubic beziers:
  - `--md-spatial-fast`: `350ms linear(0, .234 7%, .773 22%, 1.06 40%, 1.039 58%, .994 78%, 1)` for snappy micro-interactions and hover expansions.
  - `--md-spatial-bounce`: `500ms linear(...)` for celebratory pairing completions and edge crossings.
  - `--md-effects`: `180ms cubic-bezier(.2, 0, 0, 1)` for subtle opacity and color transitions.

### 4. Multi-Network Discovery Sighting Badges
To provide immediate visual clarity on how a peer device is reachable, `AddPanel.tsx` and `PeerInfo` render specialized tonal badges:
- **LAN Badge (`.sighting-badge--lan`)**: Green tonal container (`--md-badge-lan-bg`) indicating standard local subnet discovery (source 0/1/subnet).
- **Tailscale Badge (`.sighting-badge--tailscale`)**: Purple tonal container (`--md-badge-tailscale-bg`) indicating Tailscale mesh connectivity (source 3 / `100.64.0.0/10`).
- **Bluetooth PAN Badge (`.sighting-badge--bluetooth`)**: Blue tonal container (`--md-badge-bluetooth-bg`) indicating direct Bluetooth PAN point-to-point link (source 4 / `172.20.10.0/28` or link-local).

---

## 7. Settings Schema, IPC Serialization & Runtime Toggles

To provide complete user agency over all M1–M4 capabilities, Oink establishes a bidirectional settings synchronization loop between the Tauri UI and the backend daemon.

```mermaid
sequenceDiagram
    autonumber
    participant UI as Settings Page (SettingsPage.tsx)
    participant Model as Settings Model (model.ts / prefs.ts)
    participant IPC as Tauri IPC (api.ts / agent.sock / Named Pipe)
    participant Agent as Agent Daemon (agent_api.rs)
    participant Disk as Persistent Store (settings.json)
    participant Runtime as Runtime Engines (Discovery / Router)

    UI->>Model: User toggles switch (e.g. Tailscale Discovery)
    Model->>IPC: api.setInputSettings(patch: InputSettings)
    IPC->>Agent: Request {"call": "set_input_settings", "params": {...}}
    Agent->>Disk: Atomically write updated Settings to settings.json
    Agent->>Runtime: discovery.set_tailscale(enabled) / routing.set_extended_buttons(enabled)
    Runtime->>Runtime: Atomic update (AtomicBool / ArcSwap) takes effect immediately!
    Agent-->>IPC: Response {"ok": {}}
    IPC-->>Model: Hydration confirmed
    Model-->>UI: UI state committed
```

### 1. Milestone M1–M4 Settings Specification

| Setting Key (UI) | Wire JSON Field | Rust Struct Location | Default | Runtime Target & Effect |
|---|---|---|:---:|---|
| **`tailscaleDiscovery`** | `tailscale_discovery` | `crates/oink-agent/src/config.rs::Settings`<br/>`crates/oink-agent/src/api.rs::InputSettings` | `true` | **`Discovery::set_tailscale(bool)`**: Enables/disables active `tailscale status --json` querying and peer unicast UDP beaconing. |
| **`bluetoothPanDiscovery`** | `bluetooth_pan_discovery` | `crates/oink-agent/src/config.rs::Settings`<br/>`crates/oink-agent/src/api.rs::InputSettings` | `true` | **`Discovery::set_bluetooth_pan(bool)`**: Enables/disables Bluetooth PAN (`bnep0`) and link-local (`169.254.0.0/16`) subnet broadcasts and source tagging. |
| **`extendedMouseButtons`** | `extended_mouse_buttons` | `crates/oink-routing/src/config.rs::RoutingSettings`<br/>`crates/oink-agent/src/api.rs::InputSettings` | `true` | **`Router::on_input` & `inject`**: Toggles capturing, forwarding, and diagnostic telemetry for 5+ button mice (`MouseButton::Other(5..=7)`). |
| **`destroyWebViewOnClose`** | `destroy_webview_on_close` | `crates/oink-agent/src/config.rs::Settings`<br/>`crates/oink-agent/src/api.rs::InputSettings` | `true` *(macOS)* | **`apps/oink/src-tauri/src/shell.rs`**: Destroys WebView memory when settings window closes and terminates UI helper after 60s idle. |

### 2. Backward Compatibility & Serde Fallbacks
All new settings fields in `crates/oink-agent/src/api.rs` and `config.rs` utilize `#[serde(default = "default_true")]`. This ensures that existing `settings.json` files on disk generated by upstream or earlier builds deserialize safely without missing-field errors, defaulting all enhanced capabilities to active.

### 3. Frontend Settings Schema & Localization
The settings schema in `apps/oink/src/settings/schema.tsx` groups these toggles into logical domains:
- **`network` Section (`NetworkIcon`)**:
  - `tailscaleDiscovery`: Labeled *Tailscale 自动发现*, searchable with keywords *Tailscale, VPN, 跨网段, 100.64*.
  - `bluetoothPanDiscovery`: Labeled *蓝牙 PAN 与直连网卡发现*, searchable with keywords *蓝牙, PAN, 直连, 169.254, BNEP*.
- **`input` Section (`MouseIcon`)**:
  - `extendedMouseButtons`: Labeled *扩展鼠标侧键 (6–8 键)*, searchable with keywords *鼠标, 侧键, 6键, 7键, 8键*.
- **`general` Section (`GeneralIcon`)**:
  - `destroyWebViewOnClose`: Labeled *关闭窗口时释放 WebView*, configured with `only: 'mac'`.
- **Multilingual Support**: All schema labels, descriptions, and search keywords are localized across English (`en.ts`), Simplified Chinese (`zh-CN`), Japanese (`ja.ts`), and Korean (`ko.ts`).


