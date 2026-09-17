import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import { Copy, ListBullets, Power, Prohibit, XCircle } from "@phosphor-icons/react";

import type { ProcessIdentity } from "../../bindings/ProcessIdentity";
import type { SocketEntry } from "../../bindings/SocketEntry";
import type { MenuItem } from "../../components/ContextMenu";
import { describeError } from "../../lib/errors";
import { api } from "../../lib/ipc";
import { useNetwork } from "../../stores/network";
import { useProcesses } from "../../stores/processes";
import { useSettings } from "../../stores/settings";
import { toast } from "../../stores/toasts";
import { runAction } from "../actions";
import { useProcessView } from "../processes/viewState";

/** Sockets carry only a PID; actions need the start time too, so PID reuse can be detected. */
async function identityForPid(pid: number): Promise<ProcessIdentity | null> {
  const known = useProcesses.getState().byPid.get(pid);
  if (known) return { pid, startTime: known.startTime };
  try {
    const detail = await api.getProcessDetail(pid);
    return { pid, startTime: detail.info.startTime };
  } catch (error) {
    const d = describeError(error, "Looking up the owning process");
    toast({ kind: "error", title: d.title, detail: d.detail });
    return null;
  }
}

export async function quitOwner(pid: number, force: boolean): Promise<void> {
  const target = await identityForPid(pid);
  if (!target) return;
  await runAction(force ? { type: "forceKillProcess", target } : { type: "terminateProcess", target });
}

export async function blockRemoteIp(ip: string): Promise<void> {
  const outcome = await runAction({ type: "addFirewallRule", target: { type: "remoteIp", ip }, direction: "both" });
  if (outcome) void useNetwork.getState().loadFirewall();
}

export async function blockLocalPort(port: number, protocol: SocketEntry["protocol"]): Promise<void> {
  const outcome = await runAction({ type: "addFirewallRule", target: { type: "localPort", port, protocol }, direction: "inbound" });
  if (outcome) void useNetwork.getState().loadFirewall();
}

export async function showInProcesses(pid: number): Promise<void> {
  const target = await identityForPid(pid);
  if (!target) return;
  useProcessView.getState().select(target, true);
  useSettings.getState().setView("processes");
}

async function copy(text: string, what: string) {
  try {
    await writeText(text);
    toast({ kind: "success", title: `${what} copied`, detail: <span className="num">{text}</span> });
  } catch (error) {
    toast({ kind: "error", title: `Could not copy the ${what.toLowerCase()}`, detail: describeError(error).detail });
  }
}

export function processMenu(pid: number | null, name: string): MenuItem[] {
  if (pid === null) return [{ label: "Owning process unknown", onSelect: () => undefined, disabled: true }];
  return [
    { label: `Quit ${name}`, icon: <Power size={14} />, onSelect: () => void quitOwner(pid, false) },
    { label: `Force quit ${name}`, icon: <XCircle size={14} />, danger: true, onSelect: () => void quitOwner(pid, true) },
    { label: "Show in Processes", icon: <ListBullets size={14} />, onSelect: () => void showInProcesses(pid) },
  ];
}

export function socketMenu(s: SocketEntry, listening: boolean): MenuItem[] {
  const name = s.processName ?? "process";
  const items: MenuItem[] = [];
  if (listening) {
    items.push({
      label: `Block incoming on port ${s.localPort}…`,
      icon: <Prohibit size={14} />,
      danger: true,
      onSelect: () => void blockLocalPort(s.localPort, s.protocol),
    });
  } else if (s.remoteAddr) {
    const ip = s.remoteAddr;
    items.push({ label: `Block ${ip}…`, icon: <Prohibit size={14} />, danger: true, onSelect: () => void blockRemoteIp(ip) });
  }
  items.push({ type: "separator" }, ...processMenu(s.pid, name), { type: "separator" });
  if (s.remoteAddr) {
    const ip = s.remoteAddr;
    items.push({ label: "Copy remote address", icon: <Copy size={14} />, onSelect: () => void copy(ip, "Address") });
  } else {
    items.push({ label: "Copy port", icon: <Copy size={14} />, onSelect: () => void copy(String(s.localPort), "Port") });
  }
  return items;
}
