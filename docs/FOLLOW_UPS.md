# Known gaps and follow-ups

Items deliberately deferred or not yet verified. Each needs closing before the Definition of done in
`SPEC.md` is met. Update this file as items are resolved.

## Core backend (workstream A — merged)

### Needs live verification
- **Firewall rules have never been applied on any OS.** Rule text for pf, nftables/iptables and
  `netsh` is unit-tested, but install/remove needs an interactive session with the admin / polkit /
  UAC prompt on macOS, Linux and Windows.
- **Linux and Windows implementations have only been compiled and unit-tested** (parsers, table
  decoders, rule rendering). Run the app and `cargo test` on real Linux and Windows machines or CI
  runners: processes, sockets, sock_diag byte counters (Linux), IP Helper tables and extended TCP
  stats (Windows), scanner, trash, permissions.
- **Performance was measured on a dev build with opt-level 2**, not a release build (disk limits).
  Re-measure a release build; idle RSS peaked exactly at the 100 MB budget (≈25 MB of that is debug
  symbols).

### Functional gaps
- Firewall `active` state without root: pf and nftables rules can only be listed as root, so the
  status comes from Sentinel's own record. It is not reconciled after a reboot or a manual flush.
- Windows storage scanner counts hard-linked files twice.
- Windows open-file listing walks the system handle table with a 2 s cap; may be incomplete on
  busy systems.
- Linux `has_window` always returns false, so graceful quit is always SIGTERM.
- macOS trash goes through NSFileManager, so Finder's "Put Back" is not available for items
  Sentinel trashed (the audit log keeps original paths).
- macOS fan speeds are not reported (undocumented SMC keys); temperatures are.
- Windows per-connection byte counters require Sentinel to run elevated; otherwise traffic is
  interface-level only.
- `sysinfo` pinned to 0.38 because 0.39 needs rustc 1.95 (local toolchain is 1.94). Bump when the
  toolchain is updated.

## Agent layer (workstream C — paused)
- Remaining: write-tool translator, conversation engine, plan executor, safety test suite,
  keychain secrets, agent Tauri commands, chat panel / plan cards / audit log / agent settings UI.
- `src-tauri/src/commands/agent.rs` still has 14 `not_wired` commands, so the CI scaffolding gate
  stays red until this lands.
- Live provider tests pending: local Ollama (`llama3.1:8b`), Gemini and Ollama Cloud keys from the
  user.

## Frontend (workstream B — merged)

### Needs live verification
Checked in code only, because the automated session could not drive clicks or keys (no
Accessibility permission): process tree connector lines, Network "Listening" and "Firewall rules"
tabs, Storage "Largest files" tab, treemap zoom and breadcrumb animations, the geolocation database
download prompt, and first-run onboarding. Click through each by hand.

### Contract gaps — resolved by workstream E
- `SocketEntry.processStartTime` (added) lets socket actions build a `ProcessIdentity` directly;
  `socketActions.ts` uses it and only falls back to a lookup for a multi-process app group's
  ambiguous pid.
- `ItemOutcome.path` (added) lets partial trash/move results map back to their row by path instead
  of matching display labels; `storageActions.tsx` uses it.
- `NetThroughput`/`SocketEntry` `rx_bps`/`tx_bps`/`rx_total`/`tx_total` now carry doc comments
  (surfaced as JSDoc on the generated TS) stating bytes per second (not bits) and the interface
  scope, rather than a rename — avoids touching every call site for a documentation gap.

### Performance
- Idle `sentinel-app` measured 64–100 MB RSS on a dev build; WebKit content processes were not
  measured separately. Include them in the release-build measurement.

## Ship (workstream D — merged)
- No GitHub repository yet: `ci.yml`, `release.yml` and `deploy-site.yml` are unrun.
- Signing secrets to add: `APPLE_CERTIFICATE`, `APPLE_CERTIFICATE_PASSWORD`, `APPLE_ID`,
  `APPLE_PASSWORD`, `APPLE_TEAM_ID` (optional `APPLE_SIGNING_IDENTITY`); Windows certificate or
  Azure Artifact Signing (see `RELEASING.md`). Set Pages source to "GitHub Actions".
- Website docs were written from the spec: check UI labels, app-data paths and privacy claims
  against the finished app, and replace `PLACEHOLDER` capture blocks with real screenshots.
- Windows MSI rejects semver pre-release tags such as `v0.2.0-beta.1`; use numeric pre-releases.
