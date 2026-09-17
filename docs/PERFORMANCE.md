# Performance

Budget (SPEC §7): **< 3 % average CPU** for sampling at a 1 s interval with every live stream
subscribed, and **< 100 MB RAM** for the Rust process at idle.

## Method

- Machine: Apple M1, 8 cores, 16 GB RAM, macOS 26.6.
- App started with `pnpm tauri dev` on the workstream checkout (dev profile). Two builds were measured:
  the default unoptimized dev profile and the same profile with `CARGO_PROFILE_DEV_OPT_LEVEL=2`,
  which is the closest to a release build that fits the local disk budget. Release builds
  (`opt-level = 3`, LTO off) should be at or below the opt-level 2 figures.
- "All streams" runs forced `set_sampling { intervalMs: 1000, streams: [resources, processes,
  network] }` at startup through a temporary debug hook (not committed), and counted emitted events
  to confirm data flowed (after 180 s: 180 `sentinel:resources`, 179 `sentinel:processes`,
  179 `sentinel:network`, plus `sentinel:host-resolved` as reverse DNS completed). "Idle" runs used
  the default subscription (resources only, which the sampler always records for history).
- CPU: cumulative CPU time of the `sentinel-app` process (`ps -o time=`) read at the start and end
  of a 60–90 s window after a 20–30 s warm-up; average % = Δcpu-seconds ÷ window × 100 (100 % = one
  core). This avoids the decaying average `ps -o %cpu` reports on macOS.
- Memory: `ps -o rss=` every 5 s during the window (average and max), plus `vmmap --summary` /
  `footprint` for the physical footprint, which is what Activity Monitor's Memory column shows.
- WebKit renders in separate XPC processes (`com.apple.WebKit.WebContent`, `.GPU`, `.Networking`);
  they are listed separately and are not part of the Rust budget. The UI at measurement time was
  the scaffold frontend, so WebContent numbers will grow with the real UI.
- Host load during runs: normal desktop use (browser, editor, ~600 processes, ~170 sockets).

## Results

| Build | Streams @ 1 s | Avg CPU | RSS avg / max | Physical footprint |
|---|---|---|---|---|
| dev (opt-level 0), before detail caching | resources + processes + network | 3.78 % | 103.8 / 106.3 MB | — |
| dev, opt-level 2, with detail caching | resources + processes + network | **2.10 %** | 91.1 / 91.9 MB | — |
| dev, opt-level 2 | resources only (default, idle) | **0.28 %** | 88.8 / 100.0 MB | **34.1 MB** (peak 42.8 MB) |

WebKit helper processes during the all-streams run: WebContent 25.9 MB, GPU 16.9 MB, Networking
13.7 MB RSS, each under 0.5 s CPU over the run.

### Reading the memory numbers

RSS overstates the process's own memory in a dev build: `vmmap` attributes 25 MB of resident,
clean, file-backed pages to `__LINKEDIT` (the unstripped debug binary's symbol tables), which the
kernel can drop at any time and which a stripped release binary does not have. The physical
footprint — dirty memory the process actually owns — was 34 MB at idle. The mapped geolocation
database (122 MB file) contributes nothing until lookups touch its pages, and then only clean,
reclaimable pages.

## What keeps sampling cheap

- **Subscription-gated streams.** Processes and network are only sampled while a view subscribes
  through `set_sampling`; resources (a few syscalls) always run so 15 minutes of history exist.
- **Socket table without per-process walks.** macOS sockets and per-connection byte counters come
  from one read each of the `net.inet.tcp.pcblist_n` / `udp.pcblist_n` sysctls (the source of
  `netstat -anvb`) instead of `proc_pidfdinfo` on every descriptor of every process, and `nettop`
  was rejected: its streaming mode used ~128 % CPU on this machine. Linux caches the `/proc/*/fd`
  socket-inode map for 5 s and rebuilds early only when unknown inodes appear.
- **Slow-changing process details are cached.** Thread count, open descriptor/handle count and
  nice value cost one or two syscalls per process; they refresh at most every 3 s per process (new
  processes immediately). This took the all-streams run from 3.78 % to under the budget together
  with optimization.
- **Thermal sensors** (IOKit on macOS, WMI/hwmon elsewhere) refresh every 5 s.
- **Reverse DNS never blocks sampling:** lookups run on a four-thread pool with positive (30 min)
  and negative (5 min) caches; results are pushed as `sentinel:host-resolved`.
- **Process names for sockets** reuse the sampled process table when it is fresh and otherwise use a
  name-only `sysinfo` refresh at most every 2 s.
- **Bounded history:** resource ring ≤ 1 900 samples; per-process 60 s rings pruned on exit or PID
  reuse.
- **Storage scans** run on their own rayon pool only while a scan is active, collapse files under
  64 KiB per directory, and emit progress at 4 Hz and partial trees at 1 Hz.

## Re-measuring

1. Start the app with the streams you want subscribed (the Processes and Network views subscribe
   when visible).
2. `PID=$(pgrep -f 'target/.*/sentinel-app$')`, record `ps -o time= -p $PID`, wait 60 s, record it
   again; Δ seconds × 100 / 60 is the average CPU %.
3. `footprint $PID` for owned memory; `ps -o rss= -p $PID` for resident size.
