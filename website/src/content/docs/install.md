---
title: Install Sentinel
navTitle: Install
description: Pick the right build for your computer, get past first-launch warnings, and grant the permissions Sentinel can use.
section: Get started
order: 1
---

Sentinel runs on macOS 11 or later (Apple Silicon and Intel), Windows 10 and 11 (64-bit) and 64-bit
Linux. Every release is published on the
[GitHub releases page](https://github.com/rayanjainn/sentinel/releases/latest) with these files:

| File | For |
|---|---|
| `Sentinel-<version>-macos-arm64.dmg` | Macs with Apple Silicon (M1 and later) |
| `Sentinel-<version>-macos-x64.dmg` | Macs with an Intel processor |
| `Sentinel-<version>-windows-x64-setup.exe` | Windows, installs for your user account |
| `Sentinel-<version>-windows-x64.msi` | Windows, installs for all users (needs administrator approval) |
| `Sentinel-<version>-linux-x64.AppImage` | Most Linux distributions, no installation |
| `Sentinel-<version>-linux-x64.deb` | Debian, Ubuntu and their derivatives |

Each release also includes `SHA256SUMS.txt`. To check a download, compare its hash with the matching
line in that file:

```sh
# macOS
shasum -a 256 Sentinel-0.1.0-macos-arm64.dmg
# Linux
sha256sum Sentinel-0.1.0-linux-x64.AppImage
```

```powershell
# Windows (PowerShell)
Get-FileHash .\Sentinel-0.1.0-windows-x64-setup.exe -Algorithm SHA256
```

Nothing in Sentinel requires administrator rights to run. It asks for elevation only at the moment you
add or remove a firewall rule, and you can decline every permission and still use read-only monitoring.

## macOS

### Apple Silicon or Intel

Open the Apple menu and choose **About This Mac**. If the first line says **Chip: Apple M1** (or M2,
M3, and so on), download the `macos-arm64` file. If it says **Processor** followed by an Intel model,
download `macos-x64`.

The Intel build does run on Apple Silicon through Rosetta 2, but it's slower and uses more CPU to
sample your system, which is exactly what a monitor shouldn't do. Use the build that matches your Mac.

### Install

1. Open the downloaded `.dmg`.
2. Drag **Sentinel** onto the **Applications** folder.
3. Eject the disk image, then open Sentinel from Applications or Spotlight.

Run it from Applications rather than from inside the disk image. An app launched from a mounted image
runs from a read-only, randomized location and loses its permission grants between launches.

### If macOS blocks the first launch

Releases signed with a Developer ID certificate and notarized by Apple open after the usual "downloaded
from the internet" confirmation. A build that isn't notarized is stopped with a message that Apple
could not verify Sentinel is free of malware. If you trust the download (check it against
`SHA256SUMS.txt` first):

1. Click **Done**, not Move to Trash.
2. Open **System Settings > Privacy & Security** and scroll down to the **Security** section.
3. Next to the message that Sentinel was blocked, click **Open Anyway** and confirm with your password.

macOS 15 and later no longer offer the old Control-click, Open shortcut; the Privacy & Security button
is the supported route.

### Full Disk Access

macOS hides some folders from every app until you allow it: Mail, Messages, Safari data, Time Machine
settings and parts of other apps' containers in `~/Library`. Without access, a storage scan still
completes, but those folders are marked as unreadable and their size isn't counted, so totals come up
short of what Finder reports for the disk.

Sentinel checks whether it has access when it starts and shows the result on its permissions screen.
To grant it:

1. Open **System Settings > Privacy & Security > Full Disk Access**. The button on Sentinel's
   permissions screen opens this page for you.
2. Turn on **Sentinel**. If it isn't listed, click **+** and choose `/Applications/Sentinel.app`.
3. Quit and reopen Sentinel. macOS applies the change only to newly started apps.

You can turn it off again at any time in the same place.

### Administrator password prompts

Firewall rules on macOS use the built-in packet filter (`pf`). Sentinel keeps its rules in their own
anchor, `com.apple/250.sentinel`, so they never mix with rules you or other software created. Applying
or removing a rule opens the standard macOS administrator prompt.

`pf` forgets anchor rules when the Mac restarts. Sentinel remembers the rules it created and shows them
as inactive after a restart; re-applying them takes one administrator prompt.

Ending a process that belongs to another user or to the system needs privileges Sentinel doesn't have.
Those attempts fail with a clear "permission denied" message instead of pretending to work.

## Windows

### Install

- **`setup.exe`** installs Sentinel for your user account without an administrator prompt. Windows 11
  includes the Microsoft Edge WebView2 runtime Sentinel draws its interface with; on Windows 10 the
  installer downloads it if it's missing, so stay online during setup.
- **`.msi`** installs for all users of the computer and asks for administrator approval. It's the
  better fit for managed machines and deployment tools
  (`msiexec /i Sentinel-0.1.0-windows-x64.msi`).

Windows 11 on Arm runs the x64 build under emulation.

### SmartScreen warnings

Installers signed with a code signing certificate show the publisher's name. If a release is unsigned,
Microsoft Defender SmartScreen shows **Windows protected your PC**:

1. Click **More info**.
2. Check that the app name is `Sentinel-<version>-windows-x64-setup.exe` (the publisher will read
   Unknown publisher).
3. Click **Run anyway**.

Your browser may warn first that the file isn't commonly downloaded. In Edge, open the download's
**...** menu, choose **Keep**, then **Show more > Keep anyway**.

### User Account Control prompts

Sentinel runs as a normal user. Adding or removing a firewall rule starts a small elevated helper, so
Windows shows a UAC prompt for that change only. Rules are created with `netsh advfirewall` and named
`Sentinel-<id>`, so you can also see them in **Windows Defender Firewall with Advanced Security**.

Ending processes that run as administrator or as SYSTEM needs an elevated Sentinel. Without it, those
actions fail with "access denied" and nothing else happens. To manage such processes, start Sentinel
with **Run as administrator**.

## Linux

### AppImage

The AppImage bundles its libraries, including WebKitGTK, and runs without installation:

```sh
chmod +x Sentinel-0.1.0-linux-x64.AppImage
./Sentinel-0.1.0-linux-x64.AppImage
```

AppImages need FUSE 2 to mount themselves. If the file won't start and mentions `libfuse.so.2`, install
it:

| Distribution | Command |
|---|---|
| Ubuntu 24.04 and later | `sudo apt install libfuse2t64` |
| Ubuntu 22.04, Debian 12 | `sudo apt install libfuse2` |
| Fedora | `sudo dnf install fuse fuse-libs` |
| Arch Linux | `sudo pacman -S fuse2` |

Or skip FUSE by running it with `--appimage-extract-and-run`.

### Debian and Ubuntu package

Install the `.deb` with `apt` so its dependencies come along:

```sh
sudo apt install ./Sentinel-0.1.0-linux-x64.deb
```

The package depends on WebKitGTK 4.1 (`libwebkit2gtk-4.1-0`) and GTK 3, available in Debian 12, Ubuntu
22.04 and newer. Older releases don't ship WebKitGTK 4.1; use the AppImage there.

### Runtime libraries on other distributions

If you run Sentinel outside the AppImage, the WebKitGTK 4.1 runtime is the one library to check for:

| Distribution | Package |
|---|---|
| Fedora | `webkit2gtk4.1` |
| Arch Linux | `webkit2gtk-4.1` |
| openSUSE | `libwebkit2gtk-4_1-0` |

### Administrator prompts (pkexec)

Firewall changes go through `pkexec`, which asks for your password in a polkit dialog. GNOME, KDE Plasma
and most full desktops run the polkit agent that draws that dialog. On a minimal window manager, start
one first (for example `polkit-gnome` or `lxqt-policykit`), or the request fails with an
authentication error.

### nftables and iptables

Sentinel writes its rules to a dedicated nftables table, `inet sentinel`, and falls back to iptables on
systems without `nft`. Your own rules, `ufw` and `firewalld` are left untouched. A packet dropped by any
table is dropped, so a Sentinel block applies even when another firewall would allow the traffic. To see
the table yourself:

```sh
sudo nft list table inet sentinel
```

As on macOS, ending another user's process needs root, and Sentinel reports "permission denied" rather
than failing silently.

## Uninstall

| System | Remove the app | App data |
|---|---|---|
| macOS | Quit Sentinel and drag it from Applications to the Trash | `~/Library/Application Support/com.rayanjain.sentinel` |
| Windows | **Settings > Apps > Installed apps > Sentinel > Uninstall** | `%APPDATA%\com.rayanjain.sentinel` |
| Linux | Delete the AppImage, or remove the package with `apt` | `~/.local/share/com.rayanjain.sentinel` |

API keys live in your system keychain rather than in the app data folder. Delete them in Sentinel's
agent settings before uninstalling, or remove the entries for `com.rayanjain.sentinel` in Keychain
Access (macOS), Credential Manager (Windows) or your Secret Service keyring (Linux).
