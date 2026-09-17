---
title: Privacy
description: What stays on your computer, the few reasons Sentinel uses the network, and exactly what a cloud AI provider receives.
section: Trust and privacy
order: 2
---

Sentinel sees a lot about your computer: every process, connection and file. It has no account and no
sign-in, and the data it collects stays on your machine unless you choose a cloud AI provider.

## What stays on your computer

- **Live system data.** Processes, connections, CPU, memory and storage scans are read from your
  operating system and held in memory while the app runs.
- **The activity log.** The record of actions you and the agent took is a SQLite database in Sentinel's
  app data folder.
- **Settings**, including your chosen provider and model.
- **API keys**, which live in your system keychain and are never written to files or logs. See
  [where API keys are stored](../modules/agent/#where-api-keys-are-stored).

## When Sentinel uses the network

| Reason | What's sent | When |
|---|---|---|
| Geolocation database | A download request to DB-IP | Once, when you first open the world map, and again only if you ask to update it |
| Hostnames for connections | Reverse DNS queries for remote addresses, to the DNS resolver your system already uses | While the Network screen is open |
| Local Ollama | Your conversation, to the Ollama server on your own computer | When you use the agent with Ollama |
| Cloud AI provider | Your conversation and tool results (below) | Only when you send a message with a cloud provider selected |
| API key check | One lightweight request to that provider, using the key | When you save a key |

### Locations without an IP lookup service

The world map needs a location for every remote address. Sentinel doesn't call an online geolocation
service for that, which would reveal every address your computer connects to. It downloads the
[DB-IP](https://db-ip.com) IP to City Lite database once (licensed under CC BY 4.0) and looks up
addresses locally.

Your own position on the map comes from your system time zone, or a location you set yourself. Sentinel
doesn't look up your public IP address.

## What cloud providers receive

If you pick Ollama Cloud, Anthropic, OpenAI or Google Gemini, the badge in the chat names that provider,
and each message you send transmits:

- your messages and the rest of the current conversation
- descriptions of the tools the agent can use
- the results of the read tools the agent runs to answer you

Tool results are real data from your computer. Depending on the question they can include process names
and **full command lines**, file and folder paths, file sizes and dates, listening ports, remote
addresses and hostnames. Command lines sometimes contain tokens or passwords passed as arguments, so
keep that in mind before asking a cloud model about processes.

What happens to that data next is governed by the provider's API terms and your account settings with
them. Sentinel doesn't send anything to a provider you haven't selected.

If you'd rather nothing leave your computer, use a **local Ollama model**. The badge then reads "Local,
nothing leaves this machine", and the conversation never touches the internet.

## This website

This site is static and hosted on GitHub Pages. The download section asks the GitHub API for the latest
release directly from your browser, which GitHub sees like any other request to its API. The site sets
no cookies and uses no analytics; it stores the release lookup in your browser's session storage for
ten minutes and remembers your light or dark theme choice in local storage.
