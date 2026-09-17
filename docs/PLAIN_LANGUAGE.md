# Plain language and explainability

Requirement from the user: Sentinel currently reads like every other system tool. Someone looking at
it should be able to tell, in ordinary English, **what a process is, whether it is safe to quit, who
a connection is talking to, and what every number on screen actually means** — including its unit and
its scope ("is that speed my whole machine, or Wi-Fi only?").

Two limits we state plainly in the UI instead of guessing around:

1. **The OS does not attribute a connection to a browser tab.** It attributes it to a process. When
   that process is a browser renderer, say "one browser tab or site is using this", never invent a tab.
2. **HTTPS payloads cannot be read** (and Sentinel does not capture packets). Sentinel can show who
   traffic goes to, how much, and what that endpoint is usually for. It must say the contents are
   encrypted rather than implying it inspected them.

Confidence must always be visible: known facts read as facts, inferences read as inferences ("Looks
like …"), and unknowns say so while showing the raw evidence (path, parent app, user, flags).

## 1. Process explanations (backend)

New core module producing, for every process:

```
ProcessExplanation {
  headline,        // "Brave Browser — web page"
  detail,          // one or two plain sentences
  role,            // MainApp | Renderer | Extension | Gpu | NetworkService | AudioService |
                   // Utility | Helper | Daemon | Kernel | Shell | Interpreter(script) | Unknown
  category,        // Browser | UserApp | AppHelper | OsService | Kernel | Developer | Security | Unknown
  quit_safety,     // SystemCritical | OsService | AppHelper | UserApp | Background | Unknown
  quit_note,       // what actually happens if you quit it, and the better alternative
  evidence,        // ["Flag --type=renderer", "Inside Brave Browser.app", "Started by Brave Browser (pid 812)"]
  app_name,        // owning app for helpers, so the UI can group 40 helpers under one app
}
```

Derived from: executable path and bundle/app directory, command line flags, parent process chain,
owning user, and a curated catalog of well-known processes per OS. No network lookups.

- **Chromium/Electron family** (`--type=renderer` → web page; `--extension-process` → extension;
  `--type=gpu-process` → graphics; `--type=utility --utility-sub-type=…NetworkService` → network
  service; audio service; Safari's `com.apple.WebKit.WebContent` / `…Networking` equivalents).
  Helpers name their owning app.
- **Interpreters** (`node`, `python`, `java`, `ruby`, `deno`, `bun`): name the script, jar, or entry
  point being run rather than the interpreter alone.
- **OS catalogs**: macOS (`kernel_task`, `launchd`, `WindowServer`, `mds`/`mdworker`, `backupd`,
  `cloudd`, `nsurlsessiond`, `coreaudiod`, `bluetoothd`, `powerd`, `trustd`, `Dock`, `Finder`, …),
  Linux (`systemd`, `systemd-journald`, `dbus-daemon`, `NetworkManager`, `pipewire`, `Xorg`,
  `gnome-shell`, `sshd`, `polkitd`, `cron`, …), Windows (`System`, `Registry`, `csrss`, `wininit`,
  `winlogon`, `services`, `svchost` — resolve the hosted service names, `lsass`, `dwm`, `explorer`,
  `RuntimeBroker`, `SearchIndexer`, `MsMpEng`, `fontdrvhost`, …). Each entry gets a plain description
  and a quit-safety rating.

**Quit safety wording** (used in the table, the detail drawer and the confirm dialog):
- `SystemCritical` — "Don't quit this. macOS needs it; quitting can freeze or restart your Mac."
- `OsService` — "Quitting is possible; macOS will start it again. The feature it provides (Spotlight
  search) stops until then."
- `AppHelper` — "This is part of Brave Browser. Quitting closes the page it is drawing; other tabs
  keep working."
- `UserApp` — "Safe to quit. Unsaved work in it will be lost — quit from the app itself when you can."
- `Background` / `Unknown` — plainly stated, with the evidence shown.

## 2. Connection explanations (backend)

```
ConnectionExplanation { headline, detail, service, purpose, confidence, encrypted }
```

Inputs: port (well-known port table with plain names — 443 "secure web", 53 "domain name lookup",
5353 "finding devices on your network", 123 "clock sync", 22 "remote shell"), remote hostname pattern
matched against a bundled service catalog (video, CDN, push notification, OS update, analytics,
mail, messaging, cloud storage), address scope, and the owning process's explanation.

- Volume in plain words: "2.1 MB down, 340 kB up since Sentinel started watching this connection."
- Always include the encryption sentence for TLS ports.
- Analytics/telemetry endpoints are labeled as such — useful, and honest.
- Unmatched hosts: show the hostname and say Sentinel doesn't recognise it.

## 3. Field glossary and tooltips (frontend)

One glossary module: `id → { term, plain definition, unit explanation, where the number comes from,
optional caveat }`. A small `?`/`i` affordance sits next to column headers, metric labels and panel
titles; it is a real button (keyboard focusable, `aria-describedby`, works on hover, focus and tap),
never an emoji. Copy is sentence case, no jargon without an expansion.

Must cover at minimum: CPU % (and that 100% means one core, with the machine's core count named),
memory RSS vs virtual, threads, open files/handles, priority/nice, PID/PPID, process status values,
load average (with "on this 8-core Mac, 8.0 means fully busy"), swap, compressed and wired memory,
cached memory, temperature sensors (and why fans may be missing), download/upload speed — **naming
the scope: "every network interface on this Mac — Wi-Fi, Ethernet, VPN — loopback excluded" — and the
unit: kB/s = kilobytes per second, not kilobits**, per-connection rates vs cumulative bytes and why
some platforms only offer one, connection states (Established, Listening, Time-wait … in plain words),
listening port, allocated vs apparent file size, the treemap's "small files" and "other items" nodes,
duplicate groups, each cleanup category, what Move to Trash does (and that Sentinel never deletes
permanently), what a firewall rule does and how to remove it, why locations are approximate, and what
reverse DNS shows.

A searchable glossary list in Settings, generated from the same module, so nothing drifts.

## 4. Other improvements in scope

- **Group helpers by app**: "Brave Browser — 41 processes, 3.2 GB" collapsible to its helpers, in both
  the process table and the connections list.
- **Plain subtitle per process row** (the headline above), not just the executable name.
- **"Is it safe to quit?" block** in the detail drawer, and the same sentence inside the confirm dialog.
- **Per-app network summary**: "Brave Browser — 14 connections to 9 destinations, 4.2 MB down."
- **A one-line state summary** on CPU & Memory ("Busy: Brave Browser is using 180% of 8 cores") and on
  Storage ("85 GB used of 228 GB; caches and build folders account for 12 GB").
- **Storage categories explained in place** — what each cache is for and what regenerates it.
- **Units spelled out** in tooltips everywhere, compact in tables; never mix bytes and bits.
- Contract fields the frontend asked for: `SocketEntry.startTime` (so actions skip a lookup),
  `ItemOutcome.path` (so batch results map to rows exactly), and explicit byte-per-second naming or
  documentation on throughput fields.

## 5. Testing

- Classifier tables: given path + flags + parent, assert role, category and quit safety.
- Service guesser: hostname and port samples → expected label and confidence.
- Glossary completeness: every column, metric and panel id rendered in the UI has a glossary entry
  (test fails when a new field is added without one).
- No invented data: a fixture with an unknown binary must produce an "unrecognised" explanation, not
  a fabricated description.
