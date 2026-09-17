---
title: How the agent stays safe
navTitle: Agent safety
description: A plain-language account of plan, confirm and execute, and the rules that keep the agent from doing anything you didn't approve.
section: Trust and privacy
order: 1
---

An assistant that can end processes, move files and change firewall rules must be predictable. Sentinel's
agent follows one flow with no exceptions: it **gathers** real data, **proposes** a plan, and **executes**
only what you approve. This page explains each rule and why it exists.

## The short version

- The agent reads your system before it suggests anything.
- Every change it wants to make is a separate card you approve, reject or edit.
- Nothing runs without your approval, even if you tell it to "just clean everything".
- Deleting always means moving to the Trash or Recycle Bin.
- Before an approved action runs, Sentinel checks the target hasn't changed.
- Every action and every rejection is recorded.
- The agent uses the same code as the app's buttons, so it can't do anything you couldn't do yourself.

## 1. It gathers live data first

When you ask for something, the agent starts with read-only tools: listing processes, checking which
program holds a port, scanning storage. These run immediately because they can't change anything.

A change that isn't grounded in data from the current conversation is refused. If a model tries to jump
straight to "kill process 4312" without reading anything first, Sentinel rejects that request and
nothing happens. Proposals are based on what's true on your computer now, not on what a model guesses.

## 2. It proposes a plan of individual cards

Actions that change your system are write tools. When the agent calls one, the action isn't performed.
Sentinel prepares it instead, measures it against the live system, and turns it into a card showing:

- **what** will happen, such as "Move to Trash" or "Force kill"
- **what it affects**, such as the exact path, process name and PID, or address
- **the expected impact**, such as "2.3 GB, last modified 47 days ago"

You decide on each card separately: approve it, reject it, or edit it. An edited card is prepared and
checked again before you can approve it. There is no "approve all" that skips reading.

## 3. Nothing runs without your approval

This holds for every request, however it's phrased. "Just clean up everything, I trust you" produces a
plan of cards like any other request, and nothing runs until you've made your choices.

When you run the plan, only the cards you approved execute. Rejected cards, and any card you didn't
decide on, never run. There is no setting, provider or prompt that turns this off.

## 4. Deleting means the Trash

Sentinel has exactly one way to remove a file: move it to the Trash (macOS and Linux) or Recycle Bin
(Windows). There is no permanent delete anywhere in the app, for you or for the agent. If you approve
something you later want back, restore it from the Trash. The Activity screen shows which Trash items
came from agent actions.

## 5. Targets are checked again before anything runs

Time passes between a proposal and your approval, and your system keeps changing:

- **Processes.** Process IDs are reused by the operating system. Sentinel records each process's start
  time when it prepares a card, and compares it again right before acting. If the PID now belongs to a
  different process, the action is refused with "process changed". If the process already exited, it
  reports "process not found".
- **Files.** Paths are checked to still exist before they're moved.

## 6. Proposals expire

Each prepared action carries a single-use token that expires after five minutes. An old plan can't be
approved later against a system that has moved on, and the same approval can't be replayed to run an
action twice. An expired card asks you to have the agent prepare it again.

## 7. Everything is recorded

The **Activity** screen is a full audit log stored on your computer. Each entry has the time, the action,
who triggered it (you from the interface, or the agent), and the outcome, including actions that were
rejected or failed. Results include real before-and-after measurements, like "Freed 8.7 GB, storage now
at 62% used", taken from the live system rather than estimated.

## 8. The agent can only do what you can

Every agent tool calls the exact same function as the matching button in the app. There is no separate,
more powerful path for the agent. An agent-proposed "Move to Trash" goes through the same preparation,
the same confirmation data and the same checks as clicking **Move to Trash** yourself.

The part of Sentinel that talks to the model is built without the ability to execute actions at all.
Only your approval step, through the plan you review, can hand an action to the component that
performs it.

## How this is verified

These rules are enforced in code and tested on every change. Sentinel's automated test suite runs the
agent against a scripted model and checks that:

1. a write tool call never reaches execution without your approval step
2. a write tool call with no read tool earlier in the turn is refused
3. "just clean up everything" still produces a plan and zero executed actions
4. rejected and undecided actions are never executed
5. expired or reused approvals fail
6. the behavior is identical for every provider

The tests and the rest of the source are public on
[GitHub](https://github.com/rayanjainn/sentinel), so you can read exactly what the agent is allowed to
do.

## What approval can't protect against

The review step shows you precisely what will happen, but the decision is yours. Read each card. Force
killing a process loses its unsaved work, and a firewall rule can block something you rely on. That's
why those confirmations are deliberately explicit, and why a firewall rule asks you to type its target.
