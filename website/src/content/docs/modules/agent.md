---
title: AI agent
description: Choose a local or cloud model, store API keys securely, and ask Sentinel to investigate and fix things in plain English.
section: Modules
order: 5
---

The agent panel docks to the right of the window. Ask it a question about your system, or ask it to
change something, and it works from live data using the same tools as the rest of the app. It never
changes anything without your approval; [How the agent stays safe](../../agent-safety/) explains
exactly how.

## What you can ask

- "What's eating my CPU right now, and can you fix it?"
- "Kill whatever's using port 5432."
- "Free up 10 GB, but don't touch anything I've used in the last two weeks."
- "Which apps are connected to servers outside my country?"

Answers are built from tool calls against your real system, never from assumptions. The chat shows
each tool the agent used, and any change arrives as a plan of cards for you to review.

## Providers

| Provider | Runs | API key | Notes |
|---|---|---|---|
| **Ollama** | On your computer | Not needed | Sentinel detects a running Ollama install and tells you how to get it if it's missing. `llama3.1:8b` is the suggested starting model. |
| **Ollama Cloud** | Ollama's hosted models | From your ollama.com account | Same chat format as local Ollama, larger models without local hardware. |
| **Anthropic** | Anthropic API | From the Anthropic Console | Claude models. |
| **OpenAI** | OpenAI API | From the OpenAI platform dashboard | GPT models. The base URL is configurable, so OpenAI-compatible services work too. |
| **Google Gemini** | Gemini API | From Google AI Studio | Gemini models. |

The agent depends on tool calling, so choose a model that supports it. Smaller local models follow
multi-step requests less reliably than large ones; if a local model struggles, try a bigger one before
switching to the cloud.

The plan, confirm and execute flow is identical for every provider and model. Switching providers
changes who generates the text, not what the agent is allowed to do.

## Setting up a provider

In **Settings**, the agent section has three controls:

1. **Provider**, from the list above.
2. **Model**, listing the models available for that provider.
3. **API key**, for cloud providers.

When you save a key, Sentinel checks it right away with a lightweight request to the provider and shows
whether it worked. A key that fails the check isn't saved, and the message says why, such as an invalid
key or no network connection.

Each provider has its own saved key, so you can configure several and switch between them without
typing a key again.

## Where API keys are stored

Keys go straight into your operating system's credential store:

| System | Store |
|---|---|
| macOS | Keychain |
| Windows | Credential Manager |
| Linux | Secret Service (GNOME Keyring, KWallet or compatible) |

Entries use the service name `com.rayanjain.sentinel` with one account per provider. A key passes from
the settings field to the keychain once. It's never shown again, never written to a settings file and
never included in logs. Deleting a key in Settings removes it from the keychain.

## Local or cloud: the badge

The chat always shows where your messages go. With Ollama running locally, the badge reads **Local,
nothing leaves this machine**. With any other provider it names the service receiving your requests.
For exactly what a cloud provider receives, see [Privacy](../../privacy/#what-cloud-providers-receive).

## Tools the agent can use

**Read tools** run as soon as the agent calls them, because they don't change anything:

| Tool | What it reads |
|---|---|
| `get_system_overview`, `get_resource_usage` | Machine summary, CPU, memory and load |
| `list_processes`, `get_process_details` | Running processes and one process in depth |
| `list_network_sockets`, `find_port_owner` | Connections, listening ports and which process holds a port |
| `list_volumes`, `scan_storage`, `get_storage_breakdown` | Disks and where the space goes |
| `find_large_files` | Big files, optionally only those unused for a number of days (using the later of modified and accessed time) |
| `get_cleanup_suggestions`, `find_duplicate_files` | Reclaimable space and exact duplicates |
| `list_firewall_rules` | Rules Sentinel has created |

**Write tools** never run directly. Each call becomes a card in a plan:

| Tool | Proposed action |
|---|---|
| `terminate_process`, `force_kill_process` | End or force kill a process |
| `set_process_priority` | Change a process's priority |
| `move_to_trash`, `move_paths` | Move files to the Trash or to another folder |
| `block_remote_ip`, `block_local_port`, `remove_firewall_rule` | Add or remove a firewall rule |

## Reviewing a plan

When the agent wants to change something, it explains its reasoning and shows one card per action with
what it will do, what it affects and the expected impact. For each card you can:

- **Approve** it.
- **Reject** it.
- **Edit** it, for example to leave one folder out. Sentinel re-checks the edited action against your
  system before it becomes approvable.

Then run the plan. Only approved cards execute, each result is reported in the chat with real
before-and-after numbers, and the agent summarizes what happened. You can stop a response at any time.
