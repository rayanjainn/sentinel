# Sentinel shared contract

The one artifact every workstream builds against. Rust types are the source of truth; TypeScript
types in `src/bindings/` are generated from them (`cargo test --workspace`), and CI fails if the
generated files drift from the committed ones.

## Workspace layout and ownership

```
Cargo.toml                    workspace
crates/sentinel-core/         Agent A — types, provider traits, per-OS impls, services, sampler, scanner, firewall, audit
  src/model/                    contract types (process, resources, network, storage, firewall, permissions)
  src/provider.rs               per-OS traits
  src/action.rs                 Action pipeline (prepare → commit)
  src/audit.rs                  audit log types + store trait
  src/events.rs                 CoreEvent, EventSink, event names
  src/platform/{macos,linux,windows}/   OS implementations (cfg-selected)
  src/service/                  OS-agnostic services on top of the traits
  tests/contract.rs             trait-contract tests run on every CI OS
crates/sentinel-agent/        Agent C — tool registry, AgentBackend + adapters, engine, plan executor
src-tauri/                    Tauri shell: state wiring, commands, event forwarding
  src/commands/{system,process,network,storage,firewall,actions}.rs   Agent A
  src/commands/agent.rs, src/secrets.rs                                Agent C
src/                          Agent B — React app (src/features/agent/** is Agent C)
src/bindings/                 generated, do not edit
.github/, website/, docs/     Agent D (this file: integrator)
```

Deviation from the brief: backend logic lives in `crates/` rather than `src-tauri/src/system/`. Core
and agent crates have no Tauri/WebKit dependency, so they unit-test fast and can be type-checked for
Windows and Linux from any host (`cargo check --target x86_64-pc-windows-msvc`).

## Provider traits (`sentinel-core::provider`)

| Trait | Kind | macOS | Linux | Windows |
|---|---|---|---|---|
| `ResourceProvider` | read | sysinfo + host_statistics64 (compressed/wired) + IOKit HID temps via sysinfo | sysinfo + /proc/meminfo + hwmon temps/fans | sysinfo + PDH queue-length load + WMI thermal zones |
| `ProcessProvider` | read | sysinfo + libproc (fds, threads, nice) | sysinfo + /proc/[pid]/{fd,status,stat} | sysinfo + Toolhelp/NtQuery handle count + GetPriorityClass |
| `ProcessControl` | write | kill(2), NSRunningApplication terminate, setpriority | kill(2), setpriority | EnumWindows+WM_CLOSE, TerminateProcess, SetPriorityClass |
| `NetworkProvider` | read | libproc PROC_PIDFDSOCKETINFO; per-connection bytes via long-lived `nettop` stream | /proc/net/{tcp,tcp6,udp,udp6} + /proc/*/fd inodes; bytes via sock_diag tcp_info | GetExtendedTcpTable/UdpTable; GetPerTcpConnectionEStats when enabled |
| `StorageProvider` | read | lstat st_blocks, skip firmlink mirror, ~/Library/Caches etc. | lstat st_blocks, skip /proc /sys /dev /run, ~/.cache etc. | GetFileInformationByHandle + GetCompressedFileSizeW, %LOCALAPPDATA%\Temp etc. |
| `FileOps` | write | `trash` crate (NSFileManager trashItem), rename/copy, `open -R` | `trash` crate (freedesktop trash spec), FileManager1 D-Bus | `trash` crate (IFileOperation recycle), `explorer /select,` |
| `FirewallProvider` | write | pfctl anchor `com.apple/250.sentinel` via osascript admin prompt | nftables table `inet sentinel` (iptables fallback) via pkexec | `netsh advfirewall` rules `Sentinel-<id>` via UAC-elevated helper |
| `PermissionProbe` | read | TCC FDA probe (read ~/Library/Safari), osascript availability | euid, pkexec presence | token elevation, UAC |

Every implementation is real on all three OSes. `tests/contract.rs` runs the same generic checks
against `platform::current()` on each CI runner (own PID present with correct identity; killing a
dead PID → `ProcessNotFound`; stale identity → `ProcessChanged`; spawned child terminates; trash of a
temp file lands in trash and not unlinked; scan of a tree with a `chmod 000` dir completes and
reports it unreadable; loopback listener appears in `sockets()` with our PID).

## Action pipeline (the safety backbone)

```
UI button ──► prepare_action(Action) ──► ActionPreview{token} ──► confirm dialog ──► commit_action(token)
Agent write tool ──► WriteToolTranslator ──► ActionPreparer::prepare ──► Plan card ──► user decision ──► agent_execute_plan ──► ActionCommitter::commit
```

- One `Action` enum, one `ActionService` implementing both `ActionPreparer` and `ActionCommitter`.
  UI and agent reach the exact same code.
- Tokens: single-use, 5-minute expiry, bound to the normalized action and origin.
- Commit re-validates (process start time unchanged, paths still exist) and records before/after
  metrics measured on the live system.
- Every commit and every rejection writes an `AuditEntry` (SQLite in app data dir).
- `TrashPaths` is the only delete. There is no permanent delete anywhere in the codebase.
- The agent engine never holds an `ActionCommitter` (enforced by construction, verified by tests).

## Tauri commands

All commands are `async` and return `Result<T, ErrorPayload>`. Names are what the frontend `invoke`s.

### System / resources — `commands/system.rs`
| Command | Args | Returns |
|---|---|---|
| `get_system_info` | — | `SystemInfo` |
| `get_resource_history` | `windowSecs: u32` (≤ 900) | `ResourceSample[]` |
| `set_sampling` | `config: SamplingConfig` | `SamplingConfig` (clamped) |
| `get_permission_status` | — | `PermissionStatus` |
| `open_permission_settings` | `kind: PermissionKind` | `()` |

### Processes — `commands/process.rs`
| Command | Args | Returns |
|---|---|---|
| `get_process_snapshot` | — | `ProcessSnapshot` |
| `get_process_detail` | `pid: Pid` | `ProcessDetail` |
| `get_process_history` | `pid: Pid` | `ProcessHistoryPoint[]` (last 60 s) |
| `reveal_process_executable` | `pid: Pid` | `()` |

### Network — `commands/network.rs`
| Command | Args | Returns |
|---|---|---|
| `get_network_snapshot` | — | `NetworkSnapshot` |
| `get_geo_db_status` | — | `GeoDbStatus` |
| `download_geo_db` | — | `()` (progress via `sentinel:geo-db`) |
| `get_home_location` | — | `HomeLocation` |
| `set_home_location` | `location: {lat, lon, label} \| null` | `HomeLocation` |

### Firewall — `commands/firewall.rs`
| Command | Args | Returns |
|---|---|---|
| `get_firewall_status` | — | `FirewallStatus` |
| `list_firewall_rules` | — | `FirewallRule[]` (Sentinel-created only) |

### Storage — `commands/storage.rs`
| Command | Args | Returns |
|---|---|---|
| `list_volumes` | — | `VolumeInfo[]` |
| `start_scan` | `request: ScanRequest` | `ScanId` |
| `cancel_scan` | `scanId` | `()` |
| `get_scan_tree` | `query: TreeQuery` | `TreeNode` |
| `get_scan_summary` | `scanId` | `ScanSummary` |
| `start_duplicate_scan` | `scanId, minSizeBytes: u64` | `JobId` |
| `cancel_duplicate_scan` | `jobId` | `()` |
| `get_duplicate_report` | `jobId` | `DuplicateReport` |
| `reveal_path` | `path: string` | `()` |

### Actions + audit — `commands/actions.rs`
| Command | Args | Returns |
|---|---|---|
| `prepare_action` | `action: Action` | `ActionPreview` (origin = User) |
| `commit_action` | `token: string` | `ActionOutcome` |
| `reject_action` | `token: string` | `()` |
| `get_audit_log` | `query: AuditQuery` | `AuditEntry[]` |

### Agent — `commands/agent.rs`
| Command | Args | Returns |
|---|---|---|
| `agent_list_providers` | — | `ProviderDescriptor[]` |
| `agent_get_settings` | — | `AgentSettings` |
| `agent_update_settings` | `settings: AgentSettings` | `AgentSettings` |
| `agent_provider_status` | — | `ProviderStatus[]` |
| `agent_set_api_key` | `provider: ProviderId, key: string` | `KeyStatus` (validated live; invalid keys not stored) |
| `agent_delete_api_key` | `provider: ProviderId` | `()` |
| `agent_list_models` | `provider: ProviderId` | `ModelInfo[]` |
| `agent_ollama_status` | — | `OllamaStatus` |
| `agent_new_conversation` | — | `string` |
| `agent_send_message` | `conversationId, text` | `()` (stream via `sentinel:agent`) |
| `agent_cancel` | `conversationId` | `()` |
| `agent_get_transcript` | `conversationId` | `TranscriptItem[]` |
| `agent_revise_plan_action` | `planId, actionId, action: Action` | `Plan` (re-prepared, state Pending) |
| `agent_execute_plan` | `planId, decisions: ActionDecision[]` | `Plan` (then a summary turn streams) |

API keys cross the IPC boundary once (in `agent_set_api_key`) and are never returned, logged, or
written anywhere except the OS keychain (`keyring`, service `com.rayanjain.sentinel`, account per
provider).

Providers (`ProviderId`): `ollama` (local daemon, no key, "Local — nothing leaves this machine"),
`ollamaCloud` (ollama.com hosted models, API key, same `/api/chat` wire format as local), `anthropic`
(Messages API `tools`), `openai` (Chat Completions `tools`; base-URL configurable so OpenAI-compatible
providers are a descriptor entry away), `gemini` (`generateContent` `functionDeclarations`).

## Events (push, never poll)

| Event | Payload | Cadence |
|---|---|---|
| `sentinel:resources` | `ResourceSample` | sampling interval (only if subscribed) |
| `sentinel:processes` | `ProcessSnapshot` | sampling interval (only if subscribed) |
| `sentinel:network` | `NetworkSnapshot` | sampling interval (only if subscribed) |
| `sentinel:host-resolved` | `HostResolved` | as reverse DNS completes |
| `sentinel:scan-progress` | `ScanProgress` | ~4 Hz while scanning, final on completion |
| `sentinel:scan-partial` | `ScanPartial` (depth-2 tree) | ~1 Hz while scanning |
| `sentinel:scan-complete` | `ScanSummary` | once |
| `sentinel:duplicate-progress` | `DuplicateProgress` | ~4 Hz |
| `sentinel:duplicate-complete` | `DuplicateReport` | once |
| `sentinel:geo-db` | `GeoDbStatus` | during download |
| `sentinel:agent` | `AgentStreamPayload` | streaming |

The frontend declares which live streams it needs with `set_sampling` (visible views only), keeping
the sampler under the 3% CPU budget.

## Agent tools

Read (execute immediately): `get_system_overview`, `get_resource_usage`, `list_processes`,
`get_process_details`, `list_network_sockets`, `find_port_owner`, `list_volumes`, `scan_storage`,
`get_storage_breakdown`, `find_large_files` (supports `unused_for_days` using max(mtime, atime)),
`get_cleanup_suggestions`, `find_duplicate_files`, `list_firewall_rules`.

Write (always become plan cards): `terminate_process`, `force_kill_process`, `set_process_priority`,
`move_to_trash`, `move_paths`, `block_remote_ip`, `block_local_port`, `remove_firewall_rule`.

Invariant tests (`crates/sentinel-agent/tests/safety.rs`), run with a scripted mock backend:
1. A write tool call never reaches `ActionCommitter` without `agent_execute_plan`.
2. A write tool call with no prior read tool in the turn is refused.
3. "Just clean up everything" still yields a `PlanProposed` event and zero commits.
4. Rejected and omitted actions are never committed.
5. Expired or reused tokens fail with `ActionTokenInvalid`.
6. Identical behavior for every `ProviderId`.

## Decisions made while drafting

- **Geolocation DB**: DB-IP IP-to-City Lite (mmdb, CC BY 4.0) downloaded on first use. MaxMind
  GeoLite2 requires an account license key; the reader (`maxminddb` crate) accepts either file.
- **Home marker on the map**: derived from the system time zone (zone.tab coordinates) with a manual
  override — no public-IP lookup.
- **TypeScript 6.0.x** (typescript-eslint does not support 7 yet), **pnpm**, **Tauri 2.11**
  (crates.io `latest` is 3.0 alpha; pinned to 2).
- **Storage memory bound**: files under 64 KiB are collapsed per directory into a `SmallFiles` node;
  directories and larger files are kept individually. Tree slices are fetched lazily by depth.
- **Firewall persistence**: pf anchors do not survive reboot; rules are stored by Sentinel and shown
  as inactive until re-applied (one admin prompt).
- **Windows graceful kill** on a windowless process has no SIGTERM equivalent; the preview states it
  will use TerminateProcess.
- **Bundle identifier** `com.rayanjain.sentinel` (Tauri warns on identifiers ending in `.app`).
- **Scaffolding**: every Tauri command starts as `not_wired(...)` returning `Unavailable`, so the UI
  compiles and renders error states from day one. CI fails while any `not_wired` call remains.
- **Remote**: no GitHub repo yet. Workflows are written against `rayanjainn/sentinel` and verified
  once pushed.
