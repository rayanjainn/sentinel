import { ArrowClockwise, ShieldCheck, ShieldSlash, Trash } from "@phosphor-icons/react";
import { useEffect } from "react";

import type { FirewallBackend } from "../../bindings/FirewallBackend";
import type { FirewallRule } from "../../bindings/FirewallRule";
import type { TrafficDirection } from "../../bindings/TrafficDirection";
import { Button } from "../../components/Button";
import { Skeleton } from "../../components/Skeleton";
import { EmptyState, ErrorState, StatePanel } from "../../components/States";
import { cx } from "../../lib/cx";
import { formatDateTime, formatRelative } from "../../lib/format";
import { useNetwork } from "../../stores/network";
import { runAction } from "../actions";

export const BACKEND_LABEL: Record<FirewallBackend, string> = {
  pf: "Packet Filter (pf)",
  nftables: "nftables",
  iptables: "iptables",
  windowsFirewall: "Windows Defender Firewall",
};

const DIRECTION_LABEL: Record<TrafficDirection, string> = {
  inbound: "Incoming",
  outbound: "Outgoing",
  both: "Incoming and outgoing",
};

function ruleTarget(rule: FirewallRule): { text: string; mono: boolean } {
  return rule.target.type === "remoteIp"
    ? { text: rule.target.ip, mono: true }
    : { text: `Port ${rule.target.port} ${rule.target.protocol.toUpperCase()}`, mono: false };
}

/** Re-sending the stored target re-applies a rule that a restart cleared from the OS firewall. */
async function reapplyRule(rule: FirewallRule) {
  const outcome = await runAction({ type: "addFirewallRule", target: rule.target, direction: rule.direction });
  if (outcome) void useNetwork.getState().loadFirewall();
}

async function removeRule(rule: FirewallRule) {
  const outcome = await runAction({ type: "removeFirewallRule", ruleId: rule.id });
  if (outcome) void useNetwork.getState().loadFirewall();
}

function FirewallStatusLine() {
  const firewall = useNetwork((s) => s.firewall);
  const error = useNetwork((s) => s.firewallError);
  const load = useNetwork((s) => s.loadFirewall);
  if (error && !firewall) return <ErrorState error={error} subject="Firewall control" compact onRetry={() => void load()} />;
  if (!firewall) return <Skeleton className="h-3 w-72" />;
  if (!firewall.available) {
    return (
      <p className="text-[12px] text-fg-muted">
        Sentinel cannot manage firewall rules on this system{firewall.note ? `: ${firewall.note}` : "."}
      </p>
    );
  }
  return (
    <div className="flex flex-wrap items-center gap-x-5 gap-y-1 text-[12px] text-fg-muted">
      <span>
        Uses <span className="text-fg">{firewall.backend ? BACKEND_LABEL[firewall.backend] : "the system firewall"}</span>
      </span>
      {firewall.firewallEnabled !== null && (
        <span className={cx("flex items-center gap-1.5", firewall.firewallEnabled ? "text-fg-muted" : "text-warn")}>
          {firewall.firewallEnabled ? <ShieldCheck size={13} /> : <ShieldSlash size={13} />}
          {firewall.firewallEnabled ? "Firewall is on" : "Firewall is off, so rules will not take effect"}
        </span>
      )}
      {firewall.requiresElevation && <span>Changes ask for administrator authorization</span>}
      {firewall.note && <span className="text-fg-subtle">{firewall.note}</span>}
    </div>
  );
}

/** Rules Sentinel created. Other firewall rules on the system are never listed or touched. */
export function FirewallRules() {
  const rules = useNetwork((s) => s.rules);
  const status = useNetwork((s) => s.rulesStatus);
  const error = useNetwork((s) => s.rulesError);
  const load = useNetwork((s) => s.loadFirewall);

  useEffect(() => {
    void useNetwork.getState().loadFirewall();
  }, []);

  return (
    <div className="h-full overflow-y-auto">
      <div className="mx-auto flex max-w-[980px] flex-col gap-5 px-6 py-5">
        <div className="flex flex-col gap-1.5">
          <h2 className="text-[15px] font-semibold tracking-display text-fg">Rules added by Sentinel</h2>
          <p className="max-w-[70ch] text-fg-muted">
            Only rules created here are shown. Your other firewall rules are left alone. Rules that are not active
            were cleared by a restart and can be removed or added again.
          </p>
          <FirewallStatusLine />
        </div>

        {status === "loading" && rules.length === 0 && (
          <div className="flex flex-col divide-y divide-line border-y border-line">
            {Array.from({ length: 3 }, (_, i) => (
              <div key={i} className="flex items-center gap-6 py-3">
                <Skeleton className="h-3 w-40" />
                <Skeleton className="h-3 w-28" />
                <Skeleton className="ml-auto h-3 w-20" />
              </div>
            ))}
          </div>
        )}
        {status === "error" && rules.length === 0 && (
          <StatePanel>
            <ErrorState error={error} subject="Firewall rules" onRetry={() => void load()} />
          </StatePanel>
        )}
        {status === "ready" && rules.length === 0 && (
          <StatePanel className="justify-start">
            <EmptyState
              icon={<ShieldCheck size={22} />}
              title="Sentinel has not added any firewall rules"
              detail="Block a remote address from the Connections tab, or a port from the Listening tab."
            />
          </StatePanel>
        )}
        {rules.length > 0 && (
          <ul className="flex flex-col divide-y divide-line border-y border-line">
            {rules.map((rule) => {
              const target = ruleTarget(rule);
              return (
                <li key={rule.id} className="grid grid-cols-[minmax(0,1.2fr)_minmax(0,1fr)_140px_110px_minmax(210px,auto)] items-center gap-4 py-2.5">
                  <span className={cx("selectable truncate text-fg", target.mono && "num")}>{target.text}</span>
                  <span className="text-[12px] text-fg-muted">{DIRECTION_LABEL[rule.direction]}</span>
                  <span className="text-[12px] text-fg-muted" title={formatDateTime(rule.createdAtMs)}>
                    Added {formatRelative(rule.createdAtMs)}
                  </span>
                  <span className={cx("flex items-center gap-1.5 text-[12px]", rule.active ? "text-signal" : "text-fg-subtle")}>
                    <span className={cx("size-1.5 rounded-full", rule.active ? "bg-signal" : "bg-fg-subtle")} />
                    {rule.active ? "Active" : "Not active"}
                  </span>
                  <span className="flex justify-end gap-1.5">
                    {!rule.active && (
                      <Button size="sm" icon={<ArrowClockwise size={13} />} onClick={() => void reapplyRule(rule)}>
                        Apply again
                      </Button>
                    )}
                    <Button size="sm" variant="dangerQuiet" icon={<Trash size={13} />} onClick={() => void removeRule(rule)}>
                      Remove rule
                    </Button>
                  </span>
                </li>
              );
            })}
          </ul>
        )}
      </div>
    </div>
  );
}
