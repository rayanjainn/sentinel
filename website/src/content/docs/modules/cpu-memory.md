---
title: CPU and memory
navTitle: CPU and memory
description: Per-core load, memory and swap over time, load average, and temperature and fan sensors where your system exposes them.
section: Modules
order: 3
---

This screen is the long view of how hard your computer is working, with the processes responsible one
glance away.

## CPU

- **Per-core graphs.** Every logical core gets its own small live graph, so a single-threaded program
  pinning one core is as visible as a build using all of them.
- **Total CPU.** Usage across all cores combined.
- **History.** Graphs keep five minutes by default and can show up to 15 minutes. They scroll smoothly
  as samples arrive instead of jumping once a second.

## Memory

Memory is shown as a stacked area graph over time, alongside a live list of the processes using the
most.

| Band | Meaning |
|---|---|
| Used | Memory held by programs and the system that can't simply be dropped |
| Cached | File data the system keeps in RAM for speed and gives back when programs need it |
| Free | Memory not used for anything |
| Swap | Memory moved out to disk because RAM ran short |

A large cached band is healthy: the system is using spare memory to speed up file access. Growing swap
while used memory stays near the top is the sign that you're genuinely short of RAM. On macOS, Sentinel
also accounts for compressed and wired memory as reported by the kernel.

## Load

- **macOS and Linux:** the 1, 5 and 15 minute load averages, the number of processes running or
  waiting to run.
- **Windows:** Windows has no load average. Sentinel shows the processor queue length from Windows
  performance counters, which measures the same kind of pressure.

## Temperature and fans

Sensors are shown only when your system makes them available through public interfaces:

| System | Source |
|---|---|
| macOS | Hardware sensors exposed through IOKit |
| Linux | `hwmon` drivers, when your hardware has them loaded |
| Windows | WMI thermal zones, which many consumer PCs don't populate |

When a sensor isn't available, the panel says so and is hidden. Sentinel never shows zero degrees or
zero RPM for a sensor it can't read.

## Refresh rate and overhead

Samples arrive every second by default. Settings has a 500 ms high-refresh mode for watching a spike
closely. Sentinel only samples the data for views you have open, so leaving the app on another screen
doesn't cost CPU on this one.
