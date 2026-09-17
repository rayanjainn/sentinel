import { ArrowSquareOut, Copy, FolderOpen, MagnifyingGlassPlus, Trash } from "@phosphor-icons/react";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import { open } from "@tauri-apps/plugin-dialog";

import type { ActionOutcome } from "../../bindings/ActionOutcome";
import type { TreeNode } from "../../bindings/TreeNode";
import type { MenuItem } from "../../components/ContextMenu";
import { describeError } from "../../lib/errors";
import { api } from "../../lib/ipc";
import { fileManagerName } from "../../lib/platform";
import { useStorage } from "../../stores/storage";
import { toast } from "../../stores/toasts";
import { runAction } from "../actions";
import { collapseNested, type SizedPath } from "./selection";

function recordRemoved(paths: string[], outcome: ActionOutcome | null): boolean {
  if (!outcome || outcome.status === "failed") return false;
  if (outcome.status === "succeeded") {
    useStorage.getState().markRemoved(paths);
    return true;
  }
  // Partial: hide only the items the outcome confirms, matched by label.
  const done = paths.filter((p) => outcome.items.some((i) => i.success && (i.label === p || p.endsWith(i.label))));
  useStorage.getState().markRemoved(done);
  return true;
}

/** One Move to Trash action for the whole batch. Resolves true when anything was moved. */
export async function trashItems(items: SizedPath[]): Promise<boolean> {
  const paths = collapseNested(items).map((i) => i.path);
  if (paths.length === 0) return false;
  return recordRemoved(paths, await runAction({ type: "trashPaths", paths }));
}

export async function moveItems(items: SizedPath[]): Promise<boolean> {
  const paths = collapseNested(items).map((i) => i.path);
  if (paths.length === 0) return false;
  let destination: string | null;
  try {
    const picked = await open({ directory: true, multiple: false, title: "Choose where to move the selected items" });
    destination = typeof picked === "string" ? picked : null;
  } catch (error) {
    toast({ kind: "error", title: "Could not open the folder picker", detail: describeError(error).detail });
    return false;
  }
  if (!destination) return false;
  return recordRemoved(paths, await runAction({ type: "movePaths", paths, destinationDir: destination }));
}

export async function reveal(path: string): Promise<void> {
  try {
    await api.revealPath(path);
  } catch (error) {
    const d = describeError(error, `Revealing in ${fileManagerName}`);
    toast({ kind: "error", title: d.title, detail: d.detail });
  }
}

export async function copyPath(path: string): Promise<void> {
  try {
    await writeText(path);
    toast({ kind: "success", title: "Path copied", detail: <span className="num break-all">{path}</span> });
  } catch (error) {
    toast({ kind: "error", title: "Could not copy the path", detail: describeError(error).detail });
  }
}

export function isActionable(node: Pick<TreeNode, "kind">): boolean {
  return node.kind !== "smallFiles" && node.kind !== "remainder";
}

export function pathMenu(item: SizedPath & { name?: string }, extra: MenuItem[] = []): MenuItem[] {
  return [
    ...extra,
    { label: `Reveal in ${fileManagerName}`, icon: <ArrowSquareOut size={14} />, onSelect: () => void reveal(item.path) },
    { label: "Copy path", icon: <Copy size={14} />, onSelect: () => void copyPath(item.path) },
    { type: "separator" },
    { label: "Move to…", icon: <FolderOpen size={14} />, onSelect: () => void moveItems([item]) },
    { label: "Move to Trash", icon: <Trash size={14} />, danger: true, onSelect: () => void trashItems([item]) },
  ];
}

export function nodeMenu(node: TreeNode, onZoom?: () => void): MenuItem[] {
  if (!isActionable(node)) {
    return [
      {
        label: node.kind === "smallFiles" ? "Small files are grouped and cannot be acted on one by one" : "These items are too small to show here",
        onSelect: () => undefined,
        disabled: true,
      },
    ];
  }
  const zoom: MenuItem[] =
    onZoom && node.kind === "directory" ? [{ label: "Zoom into folder", icon: <MagnifyingGlassPlus size={14} />, onSelect: onZoom }, { type: "separator" }] : [];
  return pathMenu({ path: node.path, sizeBytes: node.sizeBytes, name: node.name }, zoom);
}
