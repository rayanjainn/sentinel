import { describe, expect, it } from "vitest";

import type { ActionPreview } from "../../bindings/ActionPreview";
import { confirmationPhrase, confirmLabel, doneLabel, isExpired, phraseMatches } from "./labels";

const identity = { pid: 42, startTime: 1 };

describe("action labels", () => {
  it("keeps one verb through the flow", () => {
    expect(confirmLabel({ type: "trashPaths", paths: ["/a"] })).toBe("Move to Trash");
    expect(doneLabel({ type: "trashPaths", paths: ["/a"] })).toBe("Moved to Trash");
    expect(confirmLabel({ type: "forceKillProcess", target: identity })).toBe("Force quit");
    expect(
      confirmLabel({ type: "addFirewallRule", target: { type: "localPort", port: 5432, protocol: "tcp" }, direction: "inbound" }),
    ).toBe("Block port");
  });

  it("derives the typed confirmation from the firewall target", () => {
    const base = { targets: [], title: "Block" } satisfies Pick<ActionPreview, "targets" | "title">;
    expect(
      confirmationPhrase({ ...base, action: { type: "addFirewallRule", target: { type: "remoteIp", ip: "203.0.113.9" }, direction: "both" } }),
    ).toBe("203.0.113.9");
    expect(
      confirmationPhrase({
        ...base,
        action: { type: "addFirewallRule", target: { type: "localPort", port: 8080, protocol: "tcp" }, direction: "inbound" },
      }),
    ).toBe("8080");
    expect(
      confirmationPhrase({
        title: "Remove",
        targets: [{ label: "Block 203.0.113.9", detail: null, sizeBytes: null, problem: null, safetyNote: null }],
        action: { type: "removeFirewallRule", ruleId: "r1" },
      }),
    ).toBe("Block 203.0.113.9");
  });

  it("matches typed phrases loosely on case and outer whitespace", () => {
    expect(phraseMatches("203.0.113.9", " 203.0.113.9 ")).toBe(true);
    expect(phraseMatches("8080", "808")).toBe(false);
  });

  it("detects expiry", () => {
    expect(isExpired({ expiresAtMs: 1000 }, 999)).toBe(false);
    expect(isExpired({ expiresAtMs: 1000 }, 1000)).toBe(true);
  });
});
