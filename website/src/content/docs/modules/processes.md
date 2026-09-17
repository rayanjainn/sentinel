---
title: Processes
description: Every running process with its resource use, family tree and details, and how to end or reprioritize one safely.
section: Modules
order: 1
---

The Processes screen lists everything running on your computer, refreshed every sampling interval
(one second by default). It opens as a table and switches to a tree when you want to see which process
started which.

## What each process shows

| Field | Details |
|---|---|
| PID and parent PID | The process ID and the ID of the process that started it |
| Name and command line | The executable name and the full command it was started with |
| User | The account the process runs as |
| Status | Running, sleeping, stopped or zombie |
| CPU | Current usage and a rolling average, so a brief spike doesn't hide a steady load |
| Memory | Resident memory (actually in RAM) and virtual memory (address space reserved) |
| Started and running for | Launch time and how long it has been running |
| Threads and open files | Thread count and open file descriptors (handles on Windows) |
| Priority | Nice value on macOS and Linux, priority class on Windows |
| Executable path | Where the program file lives on disk |

Sort by any column, filter the list and search to narrow it down. Rows that use a lot more CPU or memory
than the rest of your system get a subtle red tint. The threshold is relative to what else is running,
so on a busy machine only the real outliers stand out, and on an idle one a moderate process still
catches your eye.

## Tree view

The tree view shows parent and child processes with indentation and connecting lines. Collapse a
branch to hide a browser's dozens of helper processes behind one row, or expand it to find the one tab
process that's misbehaving.

## Process details

Select a process to open its detail panel:

- every field above, including the full command line
- a graph of the last 60 seconds of its CPU and memory use
- the files it has open
- its open network connections, with the same data as the [Network](../network/) screen

Some details belong to other users or to the system. macOS, Linux and Windows restrict what an ordinary
user may read from those processes, typically open files and sometimes the command line. Sentinel
shows these fields as not available instead of leaving them blank or guessing.

## Actions

Every action opens a confirmation that restates the process name and PID and says exactly what will
happen before you choose.

| Action | macOS | Linux | Windows |
|---|---|---|---|
| **End process** | Asks apps to quit normally, otherwise sends `SIGTERM` | Sends `SIGTERM` | Asks the process's windows to close; a process with no windows is ended with `TerminateProcess`, and the confirmation says so |
| **Force kill** | `SIGKILL` | `SIGKILL` | `TerminateProcess` |
| **Change priority** | Sets the nice value | Sets the nice value | Sets the priority class |
| **Reveal executable** | Selects it in Finder | Opens your file manager at it | Selects it in File Explorer |
| **Copy command line** | Copies the full command to the clipboard | same | same |

**End process** gives the program a chance to save and clean up. Use **Force kill** only when a process
ignores that request; it stops immediately and loses anything unsaved.

On macOS and Linux, lowering a process's nice value (giving it more priority) requires root. Lowering
its priority works for your own processes.

## When the process changes under you

Process IDs are recycled. If a process exits while the confirmation is open, the operating system can
hand its PID to a completely different program. Sentinel guards against killing the wrong one:

- When it prepares the action, Sentinel records the process's identity, including its start time.
- When you confirm, it checks again. If the process is gone, you get "process not found". If the PID now
  belongs to a process with a different start time, you get "process changed" and nothing is sent.
- A prepared action expires after five minutes. Confirming an expired one asks you to start over.

You can see the result of every attempt, including refused ones, on the **Activity** screen.
