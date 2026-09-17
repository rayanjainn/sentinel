import { Check, MagnifyingGlass, PencilSimpleLine, X } from "@phosphor-icons/react";

import { cx } from "../../lib/cx";
import { toolLabel } from "./model";
import type { ChatItem } from "./transcript";

type ToolItem = Extract<ChatItem, { kind: "tool" }>;

/** One line per tool call: what the agent looked at, or what change it proposed. */
export function ToolCallRow({ item }: { item: ToolItem }) {
  const finished = item.ok !== null;
  const write = item.access === "write";
  return (
    <div
      className={cx(
        "flex min-h-7 items-center gap-2 border-l-2 py-1 pl-2.5 text-[12px]",
        write ? "border-warn/60" : "border-line-strong",
      )}
    >
      <span className="relative flex size-3.5 shrink-0 items-center justify-center">
        {!finished ? (
          <>
            <span className="live-ping absolute size-1.5 rounded-full bg-signal" />
            <span className="size-1.5 rounded-full bg-signal" />
          </>
        ) : item.ok ? (
          write ? (
            <PencilSimpleLine size={13} weight="bold" className="text-warn" />
          ) : (
            <MagnifyingGlass size={13} weight="bold" className="text-fg-subtle" />
          )
        ) : (
          <X size={13} weight="bold" className="text-danger" />
        )}
      </span>
      <span className={cx("shrink-0", finished ? "text-fg-muted" : "text-fg")}>{toolLabel(item.name, finished)}</span>
      {item.summary && (
        <span
          title={item.summary}
          className={cx("min-w-0 truncate", item.ok === false ? "text-danger" : "text-fg-subtle")}
        >
          {item.summary}
        </span>
      )}
      {finished && item.ok && !write && <Check size={11} weight="bold" className="ml-auto shrink-0 text-fg-subtle" />}
    </div>
  );
}
