---
title: Getting started
description: What happens on first launch, which permissions matter, and a short tour of every part of the app.
section: Get started
order: 2
---

Sentinel reads live data straight from your operating system. There is no account, no sign-up and no
sample data: the first screen you see already shows what's running on your computer.

## First launch

The first time Sentinel opens, it shows a permissions screen. Each item explains what it unlocks and
why, and has a button that takes you to the right system setting.

- **Full Disk Access (macOS).** Lets the storage scan measure folders macOS protects, such as Mail and
  Safari data. See [Full Disk Access](../install/#full-disk-access).
- **Administrator approval for firewall rules (all systems).** Sentinel doesn't ask for this up front.
  You're prompted by macOS, Windows UAC or polkit only at the moment you add or remove a rule.

You can skip every permission. Process, network and CPU and memory monitoring work without them, and
anything that can't be read is labeled "not available" with the reason, never filled in with guesses
or zeros. You can come back to the permissions screen later from Settings.

## Finding your way around

The sidebar switches between the four monitoring views and two supporting screens:

| Screen | What it's for |
|---|---|
| **Processes** | Every running process, as a table or a parent-and-child tree |
| **Network** | Listening ports, active connections and the world map |
| **CPU and memory** | Per-core load, memory breakdown, temperatures where available |
| **Storage** | Disk scans, the treemap, largest files, duplicates and cleanup suggestions |
| **Activity** | The audit log of every action taken, by you or through the agent |
| **Settings** | Theme, refresh rate, agent provider and API keys, permissions |

The agent lives in a panel docked to the right edge of the window. Open and close it from the toolbar
while you keep working in any view.

Sentinel only samples what you're looking at. The view on screen tells the background sampler which
streams it needs, and everything else pauses. Data refreshes every second by default; a 500 ms
high-refresh mode is in Settings for when you're watching something closely.

## A short tour

These steps take about five minutes and touch every module.

1. **Processes.** Sort by CPU, then type part of a process name in the search field. Switch to the tree
   view to see which process started which, and select any row to open its detail panel with a
   60-second CPU and memory graph, open files and network connections.
2. **Network.** Look at the listening ports to see which programs accept connections. Open the world
   map: the first time, Sentinel asks to download its geolocation database, a one-time download stored
   on your computer (details in [Privacy](../privacy/)).
3. **CPU and memory.** Watch the per-core graphs while you open a heavy app, and check the memory
   breakdown to see how much is cached rather than truly in use.
4. **Storage.** Pick a disk or folder and start a scan. The treemap fills in while the scan runs;
   click any block to zoom in and use the breadcrumbs to step back out.
5. **Agent.** In Settings, pick a provider. A local Ollama model keeps everything on your computer;
   cloud providers need an API key. Then ask something read-only, such as "what's using the most
   memory right now?"

## Taking an action

Any change to your system goes through the same confirmation, whether you click a button or the agent
suggests it:

1. Sentinel prepares the action and checks it against the live system.
2. A confirmation shows exactly what will happen: the process name and PID for a kill, every path and
   the total size for a move to Trash, the address or port for a firewall rule. Firewall rules also ask
   you to type the target to confirm.
3. When you confirm, Sentinel checks again that nothing changed in the meantime, runs the action and
   reports the result with real before-and-after numbers.

The button keeps its name through the flow, so **Move to Trash** ends as **Moved to Trash**. Every
confirmed and rejected action is recorded on the **Activity** screen with the time, what was done, who
asked for it (you or the agent) and the outcome.

If something goes wrong, for example the process already exited or a file was moved while the dialog
was open, you get a specific message saying what happened and nothing else is touched.

## Next steps

- Learn each module in depth, starting with [Processes](../modules/processes/).
- Set up a provider in [AI agent](../modules/agent/).
- Read [How the agent stays safe](../agent-safety/) before asking it to change anything.
