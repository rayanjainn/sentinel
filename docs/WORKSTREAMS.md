# Parallel workstreams

Product requirements: `docs/SPEC.md`. Shared contract: `docs/CONTRACT.md` (authoritative for types,
traits, commands, events). Design system: `docs/DESIGN.md`.

## Mechanics

| Stream | Branch | Checkout |
|---|---|---|
| A — Core systems | `ws/a-core` | `.worktrees/a-core` |
| B — Frontend shell + modules 1–4 UI | `ws/b-frontend` | `.worktrees/b-frontend` |
| C — Agent layer | `ws/c-agent` | `.worktrees/c-agent` |
| D — CI/CD + website | `ws/d-ship` | `.worktrees/d-ship` |

- Work only inside your checkout. All worktrees share one Cargo target dir
  (`CARGO_TARGET_DIR=/Users/rayanjain/Projects/sentinel/target`) to save disk; a short
  "waiting for file lock" is normal.
- Commit small and often with conventional messages (`feat(core): …`, `fix(ui): …`). Each commit
  builds. Plain human-style messages under the configured git identity. **No `Co-Authored-By`,
  no "Generated with" footers, no mention of Claude/Anthropic in commits.**
- Never commit secrets, `.env`, keys, or downloaded databases.
- To pick up another stream's work: `git merge --no-edit ws/a-core` (branches share one repo).
  B and C should merge `ws/a-core` whenever they need real data; everyone may merge `main`.
- Stay inside owned paths. If you need a change in someone else's area or a contract change, make
  the smallest additive change possible, keep it compiling, and list it under "Contract changes" in
  your final report. Never rename or remove contract items unilaterally.
- Rust: `cargo fmt`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test` clean
  before each commit touching Rust. Regenerate TS bindings with `cargo test --workspace` and commit
  them with the Rust change. Frontend: `pnpm typecheck && pnpm lint && pnpm test`.
- No mock data in product code. Test doubles in tests are fine.
- No `todo!()`, `unimplemented!()`, `console.log`, or leftover `not_wired` in owned areas at the end.

## Ownership

### A — Core systems
Owns `crates/sentinel-core/**`, `src-tauri/src/commands/{system,process,network,storage,firewall,actions}.rs`,
`src-tauri/src/state.rs`, core runtime wiring in `src-tauri/src/lib.rs` `setup`.

Deliver: real macOS/Linux/Windows implementations of every trait in `provider.rs`
(`platform::{macos,linux,windows}`, selected via `cfg`), the OS-agnostic services (sampler with
subscription-gated streams + ring-buffer history, socket enrichment with async reverse DNS and local
mmdb geolocation, rayon parallel scanner with streaming partials, duplicate finder, cleanup
suggestions, `ActionService` implementing `ActionPreparer` + `ActionCommitter`, SQLite `AuditStore`,
firewall rule persistence), a runtime implementing `service::SystemQueries`, and all Tauri commands
in the owned files wired for real, with events forwarded via `EventSink`. `tests/contract.rs` per
CONTRACT.md. Verify locally on macOS by running the app; keep Windows and Linux compiling with
`cargo check -p sentinel-core --target x86_64-pc-windows-msvc` / `x86_64-unknown-linux-gnu`.
Performance budget: <3% CPU sampling at 1 s, <100 MB RSS idle — measure and record numbers in
`docs/PERFORMANCE.md`.

### B — Frontend
Owns `src/**` except `src/features/agent/**` and `src/bindings/**`.

Deliver: app shell (sidebar nav, title-bar-overlay aware header, dockable right panel that renders
`AgentPanel`, Activity route rendering `AuditLogView`, Settings view with `AgentSettingsSection`),
theming per DESIGN.md, onboarding/permissions screen, Zustand stores fed by `on(...)` events from
`src/lib/ipc.ts` with `set_sampling` subscriptions tied to visible views, and the four module UIs from
SPEC.md §2–5 (process table + tree + detail drawer; connections grouped by process + world map; CPU
per-core + memory stacked area + thermal; storage treemap with zoom/breadcrumbs, by-type, largest
files, suggestions batch actions, duplicates). A reusable action confirm flow
(`src/features/actions/`) built on `prepareAction` → dialog showing `ActionPreview` → `commitAction`,
with `ActionPreviewBody` exported for the agent's plan cards. Skeleton loading, empty, and specific
error states everywhere. Load the `design-taste-frontend` and `dataviz` skills before building UI.

### C — Agent layer
Owns `crates/sentinel-agent/**`, `src-tauri/src/commands/agent.rs`, `src-tauri/src/agent_state.rs`,
`src-tauri/src/secrets.rs`, `src/features/agent/**`.

Deliver: tool specs + JSON schemas, `ReadToolExecutor` over `SystemQueries`,
`WriteToolTranslator`, `AgentBackend` adapters (Ollama local, Ollama Cloud, Anthropic, OpenAI,
Gemini) with streaming, `AgentEngine` loop and `PlanExecutor` per the safety rules in
`crates/sentinel-agent/src/lib.rs`, keychain-backed key storage with live validation, settings
persistence, all agent Tauri commands, `tests/safety.rs` invariant suite plus adapter wire-format
tests, and the chat panel / plan cards (approve / reject / edit per action) / audit log / agent
settings UI in `src/features/agent/`.

### D — Ship
Owns `.github/**`, `website/**`, `docs/**` except CONTRACT.md, WORKSTREAMS.md, SPEC.md, DESIGN.md.

Deliver: `ci.yml`, `release.yml` (tauri-action matrix: macOS arm64 + x64, Windows x64, Linux
deb + AppImage; signing with documented secrets and graceful unsigned fallback on Windows; versioned
artifact names), `deploy-site.yml` (GitHub Pages, path-filtered on `website/**`), and the Astro site
(hero, feature cards for the five modules, OS-detected downloads pointing at
`https://github.com/rayanjainn/sentinel/releases/latest/download/<file>`, routed docs:
`/docs/install`, `/docs/getting-started`, `/docs/modules/*`, `/docs/agent-safety`, `/docs/privacy`).
Import `src/styles/tokens.css` for visual consistency. Placeholder screenshot blocks clearly marked
`PLACEHOLDER` until real captures exist.
