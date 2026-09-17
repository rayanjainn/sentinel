---
title: Network and ports
navTitle: Network and ports
description: Listening ports, live connections matched to their processes, the world map, and firewall rules you can add and remove.
section: Modules
order: 2
---

The Network screen answers two questions: which programs are accepting connections, and who is each
program talking to right now.

## Listening ports

Every listening socket is listed with its protocol (TCP or UDP, IPv4 or IPv6), local address and port,
and the process that owns it, by name and PID. This is the quickest way to find what's holding a port:
a dev server that didn't shut down, or a database you forgot was running.

## Connections

Each active connection shows:

- local and remote address and port
- the remote hostname, resolved in the background, so the address appears first and the name follows
  when the lookup returns
- the connection state, such as `ESTABLISHED`, `LISTEN` or `TIME_WAIT`
- the owning process

Connections are grouped by process, for example "Firefox, 14 connections", and each group collapses.
Filter by process, protocol or state to focus on what you're investigating.

### How sockets are matched to processes

| System | Source |
|---|---|
| macOS | Each process's socket descriptors through `libproc` |
| Linux | `/proc/net/tcp`, `tcp6`, `udp` and `udp6`, matched to processes through `/proc/<pid>/fd` |
| Windows | `GetExtendedTcpTable` and `GetExtendedUdpTable` |

On Linux, the sockets of processes owned by other users can only be matched with root access. Sentinel
still lists those sockets and marks the owner as not available.

## Throughput

The graph at the top shows machine-wide inbound and outbound traffic as it happens.

Per-connection numbers depend on what the operating system provides. Where it exposes live counters
(on macOS through a long-running `nettop` stream, on Linux through the kernel's TCP socket statistics,
on Windows through per-connection statistics when they're enabled) you get live rates. Otherwise
Sentinel shows the bytes counted since it started tracking that connection and labels them that way.

## World map

The map places remote endpoints at their approximate location with pulsing dots, and draws arcs from
your location to each one.

- **Locations are looked up on your computer.** Sentinel uses the DB-IP IP to City Lite database,
  downloaded once when you first open the map and stored locally. No lookup service sees the addresses
  you connect to. See [Privacy](../../privacy/).
- **Locations are approximate.** IP geolocation is usually accurate to a city or region, and often
  shows where a company's servers or network are registered, not where a person is.
- **Private addresses aren't plotted.** Loopback, local network and similar addresses have no location.
- **Your marker comes from your time zone.** Sentinel places "you" using the system time zone, without
  looking up your public IP address. You can set your location manually on the map.

## Actions

### End the owning process

Every connection and listening port links to its process. **End process** and **Force kill** work
exactly as on the [Processes](../processes/) screen, including the identity check against recycled PIDs.

### Block with a firewall rule

You can block a remote IP address or a local port. Because a wrong rule can cut off something you rely
on, the confirmation spells out the rule and asks you to type the address or port before the button
activates. Applying it needs administrator approval:

| System | Mechanism | Prompt |
|---|---|---|
| macOS | `pf` rules in the anchor `com.apple/250.sentinel` | macOS administrator password dialog |
| Linux | nftables table `inet sentinel`, or iptables where nftables isn't available | polkit dialog through `pkexec` |
| Windows | Windows Defender Firewall rules named `Sentinel-<id>`, created with `netsh advfirewall` | UAC prompt |

The firewall rules view lists only the rules Sentinel created, separately from your own and those of
other software, and removes them one at a time with the same kind of confirmation.

On macOS, `pf` drops anchor rules when the computer restarts. Sentinel keeps its list, shows those
rules as inactive, and re-applies them with one administrator prompt when you ask.
