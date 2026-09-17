# Sentinel — product specification

Cross-platform system observability and AI control center. This is the requirements document every
workstream builds against; `docs/CONTRACT.md` turns it into types and signatures.

## 0. What we are building

A native desktop application, **Sentinel**, giving complete, real-time, rich visual insight into
everything happening on the computer — processes, network ports/connections, CPU, memory, storage —
with the ability to act on any of it (kill a process, free a port, trash/move files), plus a built-in
AI agent that performs the same actions from natural-language requests via tool calling.

Runs on **macOS, Windows, and Linux** from one codebase, OS-specific backends behind a common
interface. Production quality: proper error handling, **no placeholder or mock data anywhere** — real
live data from the actual OS at all times.

## 1. Tech stack (non-negotiable)

- **Shell: Tauri v2** (Rust core + native webview). Not Electron — a resource monitor must not be a
  resource hog.
- **Backend: Rust.**
  - Processes: `sysinfo` as the base, supplemented per OS where it falls short (libproc on macOS,
    `/proc` on Linux, `windows` crate / WMI on Windows) for open handles, per-process ports, etc.
  - Network: socket tables mapped to owning PIDs — macOS via libproc, Linux via `/proc/net/tcp{,6}`
    and `/proc/net/udp{,6}`, Windows via `GetExtendedTcpTable` / `GetExtendedUdpTable`.
  - Storage: custom recursive walker parallelized with `rayon`, building a sized tree; fast on 500 GB+.
  - One trait per domain (`ProcessProvider`, `NetworkProvider`, `StorageProvider`, `ResourceProvider`,
    plus control traits) with one implementation per OS selected by `cfg`. This abstraction is the
    architectural backbone.
- **Frontend: React + TypeScript, Tailwind CSS, Framer Motion (`motion`), Zustand.**
- **Visualization:** `visx` for real-time CPU/memory/network graphs (smooth rolling windows);
  `d3-hierarchy` treemap custom-rendered in React/SVG (no prebuilt treemap library); custom
  collapsible process tree (not a generic table).
- **Real-time flow: Tauri events.** Rust sampling loop (default 1 s, 500 ms high-refresh option)
  pushes to the frontend. Never poll from the frontend on a timer for live data.
- **AI agent:**
  - Tool calling, **not** fine-tuning (final decision).
  - Every action is a strongly typed Rust function with a JSON schema, invocable from the UI and by
    the agent.
  - Provider-agnostic `AgentBackend` trait (`send_message`, `stream_response`,
    `supports_tool_calling`, …) with one adapter per provider translating the unified tool schema to
    the provider's format and back:
    1. **Local — Ollama** (detect; prompt if missing). Default suggestion `llama3.1:8b`.
    2. **Cloud** — Anthropic (Claude), OpenAI (GPT), Google (Gemini) at minimum, plus **Ollama Cloud**
       (added by user request). Adding another provider must be a small contained change.
    3. **Settings UI:** provider dropdown, model dropdown scoped to that provider, API key field.
       Separate keychain entry per provider so several can be configured and switched without
       re-entering keys. Keys stored in the OS keychain via `keyring` — never plaintext, never logged.
    4. Validate keys on entry with a lightweight real API call and show success/failure.
  - The plan → confirm → execute loop behaves identically for every provider/model.

## 2. Module 1 — Process monitor

**Per process:** PID, PPID, name, full command line, user, status (running/sleeping/zombie/stopped),
CPU % (instant + rolling average), memory (RSS + virtual), start time, running duration, thread
count, open FD count, nice/priority, executable path.

**UI:**
- Sortable, filterable, searchable live table (default view).
- Toggleable tree view of parent/child relationships, collapsible, with indentation and connecting
  lines.
- Click a process → side detail panel: full metadata, 60 s mini CPU/memory graph for that process,
  open file handles, open network connections.
- Color-code by resource intensity using a relative threshold (e.g. subtle red tint), not a hardcoded
  number.

**Actions** (each behind a confirm dialog restating name/PID and exactly what happens):
kill (SIGTERM or graceful window close), force kill (SIGKILL), change priority/nice, reveal
executable in Finder/Explorer, copy full command line.

## 3. Module 2 — Network / ports

**Data:**
- Every listening port: protocol, local address, owning process (name + PID, cross-referenced).
- Every active connection: local and remote address:port, reverse-resolved remote hostname
  (async, never blocks UI), state (ESTABLISHED, LISTEN, TIME_WAIT, …), owning process.
- Live per-connection throughput where the OS exposes it; otherwise cumulative bytes since Sentinel
  started tracking the connection.
- Machine-wide inbound/outbound throughput, graphed live.

**UI:**
- Default: connections table groupable by owning process ("Chrome — 14 connections", collapsible).
- **World view:** 2D world map plotting approximate geolocation of remote endpoints from a **local**
  IP-geolocation database (downloaded on first run; never an external API per lookup) as pulsing
  dots with animated arcs from "you". The single biggest "whoa" moment — spend real design effort.
- Filter by process, protocol, state.

**Actions:** kill owning process (reuses Module 1); add a firewall rule blocking a remote IP or a
local port (pf on macOS, nftables/iptables on Linux, `netsh advfirewall` on Windows) behind a clear,
serious confirm dialog, with a view to list/remove rules Sentinel added, separate from the user's
other rules.

## 4. Module 3 — CPU & memory

**Data:** per-core usage with rolling graph (default 5 min, adjustable); aggregate CPU; memory
used/free/cached/swap as a stacked area graph plus a live top-memory process list; CPU temperature and
fan speed only if exposed via public APIs (hide the panel with a note otherwise — never fake zeros);
load average (Unix) / equivalent on Windows.

**UI:** the classic Activity Monitor screen, but smoother animation, better color grading, and a
per-core view that feels alive (each core its own small animated bar/graph).

## 5. Module 4 — Storage

**Data:** full disk usage from root/home recursively; breakdown by folder (treemap), by
type/extension, largest 100 files; hash-based duplicate detection (explicit user-triggered scan);
"safe to consider" categories — OS/app caches, logs, trash contents, old downloads,
node_modules/build artifacts — clearly labeled as suggestions, never automatic deletions.

**UI:** treemap is the centerpiece — colored by file type, sized by usage, click to zoom into a
folder, breadcrumbs back out, smooth animated zoom transitions (quality bar: DaisyDisk, WizTree,
GrandPerspective — then exceed them). Hover tooltip: full path, exact size, last modified, item count.
Live progress while scanning with partial results streaming in; never block on completion.

**Actions:**
- Delete → **always OS trash / recycle bin, never permanent unlink.** Hard requirement.
- Move to a destination chosen with the native picker.
- Reveal in Finder/Explorer.
- Batch-select suggestions and trash/move together with a summary confirm (total size, item count).

## 6. Module 5 — AI agent

Dockable/toggleable chat panel inside the app.

**Flow for every agent action:**
1. Plain-English request ("free up 10GB but don't touch anything I've used in the last 2 weeks",
   "kill whatever's using port 5432", "what's eating my CPU right now and can you fix it").
2. Agent calls read-only tools first to gather real current data — never reasons from assumptions.
3. Agent produces a **plan**: plain-language explanation + explicit action cards (what, what it
   affects, estimated impact — e.g. "Delete `~/Library/Caches/old-app` — 2.3 GB, last modified 47
   days ago").
4. Each card individually approved, rejected, or edited — not one global confirm.
5. Only approved actions execute. Per-action results reported in chat with real before/after numbers
   ("Freed 8.7 GB — storage now at 62% used").
6. Full audit log screen: timestamp, action, trigger, outcome (also identifies which trash items came
   from agent actions).

**Tools:** every agent tool is the exact same function the UI buttons call — no parallel code path.
Read tools run without confirmation. Write tools (kill, trash, move, firewall, priority) always go
through plan → confirm → execute, with zero exceptions, even for blanket-permission requests.

**Mode indicator:** chat always shows where data goes — local badge (private/offline) or "requests
sent to <provider> API".

## 7. Cross-cutting

- **Design:** dark by default, light supported, considered palette and type (not default grays).
  Purposeful motion: numbers tween, panels transition smoothly, skeleton loading states (never a bare
  spinner). See `docs/DESIGN.md`.
- **Performance:** under 100 MB RAM idle, under 3% average CPU for sampling. Profile explicitly.
- **Permissions:** real onboarding screen explaining why elevated access helps (macOS Full Disk
  Access, admin for firewall). Read-only monitoring works if declined; degrade gracefully.
- **No mock data ever.** Unavailable data sources get a clear "not available on this system" note.
- **Testing:** unit tests for all backend logic, especially trait-contract tests per OS; agent
  integration tests proving no destructive tool executes without confirmation.
- **Errors:** every failure (permission denied, process gone, disk unmounted mid-scan, …) surfaces a
  clear, specific message — never silent, never a raw stack trace.

## 8. Git conventions

- Small, frequent, buildable commits with conventional prefixes (`feat:`, `fix:`, `refactor:`,
  `chore:`, `docs:`, `ci:`).
- **No Claude/Anthropic co-author, trailer, or generated-with footer in any commit.** User's git
  identity only, human-style messages.
- `.gitignore` covers build output, node_modules, OS junk, secrets from the first commit.

## 9. CI/CD

- **`ci.yml`** on every push/PR: `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test` on
  macos-latest, windows-latest, ubuntu-latest; frontend `tsc --noEmit`, lint, unit tests. Warnings
  fail the build.
- **`release.yml`** on `v*` tags: `tauri-apps/tauri-action` matrix — macOS Apple Silicon and Intel,
  Windows x64, Linux `.deb` + `.AppImage`; GitHub Release with all artifacts. Code signing set up
  properly: macOS Developer ID + notarization (`APPLE_CERTIFICATE`, `APPLE_CERTIFICATE_PASSWORD`,
  `APPLE_ID`, `APPLE_PASSWORD`, `APPLE_TEAM_ID`); Windows signed if a certificate is available, else
  unsigned with a clear note (never fail the pipeline). TODO markers for secrets the user must obtain.
  Clear versioned artifact names (`Sentinel-0.1.0-macos-arm64.dmg`, `Sentinel-0.1.0-windows-x64.msi`,
  `Sentinel-0.1.0-linux-x64.AppImage`). Well-commented workflows.
- GitHub repo `rayanjainn/sentinel` is not created yet (user decision); workflows are verified once
  pushed.

## 10. Website

Separate static site in `website/` (Astro), deployed independently:
- Hero: one-sentence pitch — "see everything happening on your machine, and control it with plain
  English" — plus supporting paragraph and real UI captures (placeholders clearly flagged until then).
- Feature cards for the five modules, each with a one-line description and icon.
- Download section: detect visitor OS (with manual override), prominent matching button, other
  platforms below; links to latest GitHub Release artifacts
  (`github.com/rayanjainn/sentinel/releases/latest/download/<file>`).
- Real docs as routed pages: install per platform (incl. first-run permission steps such as macOS
  Full Disk Access), getting started / per-module walkthrough, and a plain-language explanation of the
  agent's plan → confirm → execute safety model.
- Same visual language as the app (dark-first, same palette).
- GitHub Pages deploy via `deploy-site.yml`, triggered on `website/**` changes.

## Definition of done

- All five modules show live, real, correctly labeled data on the machine running them.
- Every action (kill, free port, trash/move, firewall block, priority change) works with a visible,
  correct result.
- The agent completes each of the three example requests end to end, including the confirm step,
  with real tool calls against real state, tested with at least one local and one cloud provider.
- No crash when: killing a process that no longer exists; scanning a disk with a permission-restricted
  folder; losing network connectivity mid-scan.
- Real, non-stubbed implementations for macOS, Windows, and Linux.
- `ci.yml` passes on all three OS runners; `release.yml` produces installable artifacts for macOS
  (both architectures), Windows, and Linux from a tag.
- Website builds and deploys independently, accurate feature descriptions, working OS-detected
  download buttons, non-stub install / getting started / agent safety docs.
- Git history: small, frequent, human-authored commits; no Claude/Anthropic attribution anywhere.
