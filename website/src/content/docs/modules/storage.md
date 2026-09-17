---
title: Storage
description: Scan a disk or folder, explore it as a treemap, find large and duplicate files, and move what you don't need to the Trash.
section: Modules
order: 4
---

The Storage screen shows where your disk space went and gives you safe ways to get it back. Nothing is
ever deleted permanently: every removal goes to the Trash or Recycle Bin.

## Scanning

Choose a volume or a folder and start a scan. Sentinel walks the file tree on all CPU cores in
parallel, and you don't wait for it to finish:

- progress updates several times a second
- the treemap fills in with partial results about once a second
- you can explore, zoom and cancel while it runs

Sizes are what files actually occupy on disk, so sparse and compressed files count for their real
footprint. System locations that would double-count or aren't real files are skipped: the firmlinked
data volume mirror on macOS, and `/proc`, `/sys`, `/dev` and `/run` on Linux.

Folders you don't have permission to read are marked as unreadable and the scan carries on. If a disk
is unplugged or unmounted mid-scan, the scan stops with a message saying so and keeps the results it
already has. On macOS, granting [Full Disk Access](../../install/#full-disk-access) lets Sentinel read
folders the system protects.

To keep memory use predictable on very large disks, files smaller than 64 KiB are grouped into a single
"small files" block per folder. Folders and larger files are always shown individually.

## Treemap

Every rectangle is a folder or file, sized by the space it uses and colored by file type.

- **Click** a folder to zoom in, with a smooth transition that keeps your place.
- **Breadcrumbs** above the map take you back out to any parent.
- **Hover** for the full path, exact size, last modified date and item count.

## Other views

- **By type** totals space per kind of file, such as video, images, archives or code.
- **Largest files** lists the 100 biggest files in the scan.

## Duplicates

Duplicate detection is a separate scan you start yourself, because it reads file contents. It considers
files above a minimum size you choose, compares files of identical size by hashing their contents, and
groups exact copies. You pick which copies to move to the Trash; Sentinel never picks for you.

## Cleanup suggestions

Sentinel points out space that's often safe to reclaim, clearly labeled as suggestions:

| Category | Examples |
|---|---|
| Caches | `~/Library/Caches` on macOS, `~/.cache` on Linux, `%LOCALAPPDATA%\Temp` on Windows |
| Logs | Application and system log files |
| Trash | What's already in the Trash or Recycle Bin |
| Old downloads | Files in Downloads you haven't opened in a long time |
| Build artifacts | `node_modules` folders and other build output that can be regenerated |

Nothing in these lists is removed automatically. Select the items you want, and a summary confirmation
shows the total size and item count before anything moves.

## Actions

| Action | What happens |
|---|---|
| **Move to Trash** | Items go to the Trash on macOS, the Recycle Bin on Windows, or the freedesktop Trash on Linux. Restore them with your file manager's Put Back or Restore. |
| **Move to** | Moves items to a folder you choose with the system's folder picker. |
| **Reveal** | Shows the item in Finder, File Explorer or your Linux file manager. |

There is no permanent delete anywhere in Sentinel, for you or for the agent. Remember that items in the
Trash still take up disk space until you empty it; the before-and-after numbers Sentinel reports are
measured on the live disk and reflect that.

Before a move runs, Sentinel checks that every path still exists as it did when you reviewed it. The
**Activity** screen records each move, including which Trash items came from agent actions, so you can
find and restore them later.
