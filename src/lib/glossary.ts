// Single source of truth for every "what does this mean" tooltip in the app, plus the searchable
// list in Settings. See docs/PLAIN_LANGUAGE.md §3. Copy is sentence case, no jargon without an
// expansion, and states scope + unit wherever a number could be misread.
export interface GlossaryContext {
  /** Logical core count, when known, for the entries that name it. */
  cores: number | null;
}

export interface GlossaryEntry {
  term: string;
  /** Plain definition. A function can fold in live context, e.g. this machine's core count. */
  definition: string | ((ctx: GlossaryContext) => string);
  /** Explicit unit, spelled out, when the field is a number that could be misread. */
  unit?: string;
  /** Where the number comes from, when that matters (a sensor, a kernel counter, a heuristic). */
  source?: string;
  /** A caveat worth surfacing every time (e.g. "not available on every platform"). */
  caveat?: string;
}

export function resolveDefinition(entry: GlossaryEntry, ctx: GlossaryContext): string {
  return typeof entry.definition === "function" ? entry.definition(ctx) : entry.definition;
}

const coresPhrase = (cores: number | null) =>
  cores ? `this Mac has ${cores} logical cores, so the number can go up to ${cores * 100}%` : "one core is fully busy at 100%";

const ENTRIES = {
  // CPU & memory (machine-wide) ---------------------------------------------------------------------
  cpuTotal: {
    term: "CPU",
    definition: (ctx) =>
      ctx.cores
        ? `Every logical core on this Mac added together and averaged. 100% means all ${ctx.cores} cores are fully busy.`
        : "Every logical core on this machine added together and averaged. 100% means every core is fully busy.",
  },
  // Processes ------------------------------------------------------------------------------------
  cpuPercent: {
    term: "CPU %",
    definition: (ctx) =>
      `Share of one CPU core this process is using right now. 100% means one whole core is busy — ${coresPhrase(ctx.cores)}.`,
    source: "Sampled once per second from the operating system's own process accounting.",
  },
  cpuPercentAvg: {
    term: "Avg CPU",
    definition:
      "A rolling average of CPU %, smoothed over roughly the last 10 readings so a brief spike doesn't dominate the number.",
  },
  memoryRss: {
    term: "Memory",
    definition:
      "Physical RAM this process is actually using right now (its resident set size). This is usually the number that answers \"is this using a lot of memory?\"",
    unit: "Bytes, shown as KB/MB/GB.",
  },
  memoryVirtual: {
    term: "Virtual memory",
    definition:
      "The total address space the process has reserved, including memory mapped from files and shared libraries it may never fully use. Normally much larger than its real memory use and not a sign of trouble on its own.",
    unit: "Bytes, shown as KB/MB/GB.",
  },
  threadCount: {
    term: "Threads",
    definition: "Separate lines of execution running inside this process. More threads usually means more parallel work, not more resource use by itself.",
    caveat: "Not reported for every process without administrator access.",
  },
  fdCount: {
    term: "Open files",
    definition: "Files, sockets, pipes and other handles this process currently has open at the same time.",
    caveat: "Not reported for every process without administrator access.",
  },
  nice: {
    term: "Nice",
    definition:
      "Scheduling priority on a scale from -20 (goes first when the system is busy) to 19 (goes last). 0 is normal for most programs. On Windows this maps onto the same scale from the process's priority class.",
  },
  pid: {
    term: "PID",
    definition:
      "Process ID: a number the operating system hands out when a process starts. PIDs get reused, so Sentinel also checks when a process started before acting on it, to avoid hitting the wrong one.",
  },
  ppid: {
    term: "Parent process",
    definition: "The process that started this one — the PID column names it as \"PPID\".",
  },
  processStatus: {
    term: "Status",
    definition:
      "Running (using the CPU right now), Sleeping or idle (waiting for something, not using the CPU), Waiting (blocked on disk or network), Stopped (paused, usually by a debugger), Zombie (finished but the parent hasn't collected its exit code yet), or Exited.",
  },
  startTime: {
    term: "Started",
    definition: "When this process launched.",
  },
  runTime: {
    term: "Running for",
    definition: "How long this process has been running since it started.",
  },
  // Network ---------------------------------------------------------------------------------------
  throughputRate: {
    term: "Download / upload speed",
    definition:
      "Every network interface on this Mac added together — Wi-Fi, Ethernet, VPN. Loopback (traffic that never leaves this machine) is excluded, so it never inflates the number.",
    unit: "kB/s = kilobytes per second (1,000 bytes), not kilobits. Ten times smaller than what an ISP usually advertises in Mbps.",
    source: "The operating system's per-interface byte counters, sampled once per second.",
  },
  connectionRate: {
    term: "Rate",
    definition: "Live speed for this one connection since the previous sample, on platforms that expose per-connection counters.",
    unit: "kB/s = kilobytes per second, not kilobits.",
    caveat: "Windows only reports this while Sentinel runs as administrator; otherwise only the machine-wide total is available.",
  },
  cumulativeBytes: {
    term: "Transferred",
    definition:
      "Total bytes sent or received since Sentinel started watching this connection — not since the connection opened, and not since the machine started.",
    unit: "Bytes, shown as kB/MB/GB (1,000-based, matching Finder/Explorer).",
    caveat: "Shown instead of a live rate on platforms that only expose a running total, not a per-connection speed.",
  },
  connectionState: {
    term: "State",
    definition:
      "Established (data can flow both ways), Listening (waiting for an incoming connection), Opening (a connection is being set up), Closing (either side is shutting it down) or Time wait (closed, but kept briefly in case a stray packet still arrives).",
  },
  listeningPort: {
    term: "Port",
    definition: "The local port number a program is accepting connections on. Other programs — on this machine or the network — connect to a specific port to reach it.",
  },
  localBinding: {
    term: "Listening on",
    definition:
      "\"All interfaces\" means any device on the network can connect (not only this Mac). \"This computer only\" means it only accepts connections that start on this same machine.",
  },
  remoteHost: {
    term: "Remote host",
    definition:
      "The name behind the remote address, found by asking the network \"what name goes with this address?\" (reverse DNS). Many addresses have no such name registered, in which case Sentinel shows \"No hostname\" rather than guessing one.",
    caveat: "Resolved in the background; a new connection often shows \"Resolving…\" for a few seconds first.",
  },
  geoLocation: {
    term: "Location",
    definition:
      "An approximate city and country for a public address, from a location database installed once on this machine — never sent to an online lookup service. Large services place servers in many cities, so the plotted point is often nearest to the network, not the company's headquarters.",
    caveat: "Approximate. Never exact enough to identify a building.",
  },
  protocol: {
    term: "Proto",
    definition: "TCP (a two-way, connection-based stream) or UDP (single packets sent without a lasting connection).",
  },
  // CPU & memory ------------------------------------------------------------------------------------
  loadAverage: {
    term: "Load average",
    definition: (ctx) =>
      ctx.cores
        ? `Work queued for the CPU, averaged over the last 1, 5 and 15 minutes. On this ${ctx.cores}-core Mac, a load of ${ctx.cores.toFixed(1)} means the CPU is exactly fully busy; higher means work is waiting its turn.`
        : "Work queued for the CPU, averaged over the last 1, 5 and 15 minutes. A load equal to the number of logical cores means the CPU is exactly fully busy.",
  },
  appMemory: {
    term: "App",
    definition:
      "Regular memory used by apps and their normal work, separate from the memory the kernel has pinned (wired) or compressed. This is usually the segment that grows when you open more apps.",
    unit: "Bytes, shown as MB/GB.",
  },
  swap: {
    term: "Swap",
    definition:
      "Memory the operating system moved out of RAM onto disk because RAM filled up. A little swap use now and then is normal; constant swapping makes everything feel slow because disk is far slower than RAM.",
    unit: "Bytes, shown as MB/GB.",
  },
  compressedMemory: {
    term: "Compressed",
    definition: "Memory macOS compressed in place, without writing to disk, so more programs fit in RAM at once. Faster to bring back than memory that had to swap to disk.",
    unit: "Bytes, shown as MB/GB.",
    caveat: "macOS only. Linux and Windows don't separate this out.",
  },
  wiredMemory: {
    term: "Wired",
    definition: "Memory the kernel has pinned in RAM — it can never be moved to disk or compressed, so it is never counted as available.",
    unit: "Bytes, shown as MB/GB.",
    caveat: "macOS calls this \"wired\"; Windows' closest equivalent is its non-paged pool.",
  },
  cachedMemory: {
    term: "Cached",
    definition:
      "Recently used files the operating system is keeping in RAM in case they're read again soon. Not memory that's stuck — it's handed back instantly the moment a program actually needs it.",
    unit: "Bytes, shown as MB/GB.",
  },
  memoryUsed: {
    term: "Used",
    definition: "Memory actively in use by running programs — the app, wired and compressed segments together, not counting cache that would be freed on demand.",
    unit: "Bytes, shown as MB/GB.",
  },
  temperature: {
    term: "Temperature",
    definition: "A hardware sensor's current reading, in Celsius, read through the operating system's public sensor interface.",
    unit: "°C.",
    caveat: "Only sensors the OS exposes publicly are shown; a component can run hot without a readable sensor.",
  },
  fanSpeed: {
    term: "Fan",
    definition:
      "A cooling fan's current speed. Many Apple Silicon Macs are fanless by design and simply have none to show; on machines that do have one, some don't report its speed through any interface Sentinel can read.",
    unit: "RPM (revolutions per minute).",
  },
  perCoreUsage: {
    term: "Core usage",
    definition: "Usage of one logical core. 100% means that specific core is fully busy right now — it says nothing about the other cores, which can be idle at the same time.",
  },
  // Storage ----------------------------------------------------------------------------------------
  allocatedSize: {
    term: "Size on disk",
    definition:
      "The space a file actually occupies on disk, in whole disk blocks — what Sentinel shows everywhere it lists a size, and what you actually get back by removing it. Usually close to the file's exact byte count (its \"apparent size\"), but can be smaller for a sparse file, or rounded up on a disk with a large block size.",
  },
  smallFilesNode: {
    term: "Small files",
    definition: "Files under 64 KB inside this folder, grouped into one item so Sentinel doesn't have to track every tiny file individually in the map.",
  },
  otherItemsNode: {
    term: "Other items",
    definition: "Items too small to draw individually at this zoom level, added together into one block. Zoom in for the individual items.",
  },
  duplicateGroup: {
    term: "Duplicate group",
    definition: "Files with byte-for-byte identical contents, found by comparing a hash of each file's data. Only files within this one scan are compared against each other.",
  },
  cleanupAppCache: {
    term: "App caches",
    definition: "Files apps store to avoid redoing work — images, compiled data, temporary state. Apps rebuild what they need automatically; removing these just means the next use is a little slower once.",
  },
  cleanupLogs: {
    term: "Logs",
    definition: "Diagnostic text files apps and the system write while running. Useful only when troubleshooting a specific problem; otherwise safe to clear.",
  },
  cleanupTrash: {
    term: "Trash",
    definition: "Items already sitting in the Trash, shown for information. Sentinel never empties the Trash itself — emptying it is permanent, and that choice stays yours.",
  },
  cleanupOldDownloads: {
    term: "Old downloads",
    definition: "Files in Downloads not modified or opened in the last 90 days — often installers and archives that already did their job.",
  },
  cleanupBuildArtifacts: {
    term: "Build artifacts",
    definition: "Output folders (like node_modules or a target/build directory) that a project's build tool regenerates on demand, so they cost nothing to remove.",
  },
  cleanupPackageManagerCache: {
    term: "Package manager caches",
    definition: "Downloaded packages kept by tools like npm, Cargo or Homebrew so the next install is faster. Safe to clear; the next install just re-downloads them.",
  },
  cleanupDeveloperCache: {
    term: "Developer caches",
    definition: "Build and tooling caches from development tools (Xcode, simulators, downloaded models) that regenerate the next time that tool runs.",
  },
  moveToTrash: {
    term: "Move to Trash",
    definition:
      "Sentinel's only way to remove anything: items go to the OS Trash / Recycle Bin, where they can be put back. Sentinel never deletes a file permanently — the space is only released once the Trash itself is emptied, which Sentinel never does automatically.",
  },
  firewallRuleGlossary: {
    term: "Firewall rule",
    definition:
      "An instruction telling the operating system's firewall to block traffic to or from a specific address or port. Sentinel only shows and removes rules it created itself here; your other firewall rules are left untouched. Remove a rule from the Firewall rules tab to let that traffic through again.",
  },
} satisfies Record<string, GlossaryEntry>;

export type GlossaryId = keyof typeof ENTRIES;

// `satisfies` above keeps each entry's narrow literal type (so a typo in a key is still caught),
// which drops entries' unused optional fields from their inferred type. Re-widen to the full
// interface so every entry — even ones without a `unit`/`source`/`caveat` today — can gain one
// later without every read site needing a type guard.
export const GLOSSARY: Record<GlossaryId, GlossaryEntry> = ENTRIES;

export const GLOSSARY_IDS = Object.keys(GLOSSARY) as GlossaryId[];
