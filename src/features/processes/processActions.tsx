import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import { Copy, FolderOpen, Gauge, Power, XCircle } from "@phosphor-icons/react";

import type { ProcessIdentity } from "../../bindings/ProcessIdentity";
import type { ProcessInfo } from "../../bindings/ProcessInfo";
import type { MenuItem } from "../../components/ContextMenu";
import { describeError } from "../../lib/errors";
import { api } from "../../lib/ipc";
import { fileManagerName } from "../../lib/platform";
import { shellJoin } from "../../lib/shell";
import { toast } from "../../stores/toasts";
import { runAction } from "../actions";
import { openPriorityDialog } from "./PriorityDialog";

export const identityOf = (p: Pick<ProcessInfo, "pid" | "startTime">): ProcessIdentity => ({
  pid: p.pid,
  startTime: p.startTime,
});

export const quitProcess = (p: ProcessInfo) => runAction({ type: "terminateProcess", target: identityOf(p) });
export const forceQuitProcess = (p: ProcessInfo) => runAction({ type: "forceKillProcess", target: identityOf(p) });

export async function revealExecutable(p: ProcessInfo): Promise<void> {
  try {
    await api.revealProcessExecutable(p.pid);
  } catch (error) {
    const d = describeError(error, `Revealing ${p.name} in ${fileManagerName}`);
    toast({ kind: "error", title: d.title, detail: d.detail });
  }
}

export async function copyCommandLine(p: ProcessInfo): Promise<void> {
  const text = p.cmd.length > 0 ? shellJoin(p.cmd) : (p.exe ?? p.name);
  try {
    await writeText(text);
    toast({ kind: "success", title: "Command line copied", detail: <span className="num break-all">{text.length > 140 ? `${text.slice(0, 140)}…` : text}</span> });
  } catch (error) {
    toast({ kind: "error", title: "Could not copy the command line", detail: describeError(error).detail });
  }
}

export function processMenuItems(p: ProcessInfo): MenuItem[] {
  return [
    { label: "Quit", icon: <Power size={14} />, onSelect: () => void quitProcess(p) },
    { label: "Force quit", icon: <XCircle size={14} />, danger: true, onSelect: () => void forceQuitProcess(p) },
    { label: "Change priority…", icon: <Gauge size={14} />, onSelect: () => openPriorityDialog(p) },
    { type: "separator" },
    { label: `Reveal in ${fileManagerName}`, icon: <FolderOpen size={14} />, disabled: !p.exe, onSelect: () => void revealExecutable(p) },
    { label: "Copy command line", icon: <Copy size={14} />, onSelect: () => void copyCommandLine(p) },
  ];
}
