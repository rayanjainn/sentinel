import { File, Folder, Rows } from "@phosphor-icons/react";
import { useMemo } from "react";

import type { ExtensionStat } from "../../bindings/ExtensionStat";
import type { FileKindGroup } from "../../bindings/FileKindGroup";
import type { TreeNode } from "../../bindings/TreeNode";
import { openContextMenu } from "../../components/ContextMenu";
import { formatBytes, formatCount, formatPercent } from "../../lib/format";
import { FILE_KIND_ORDER, FILE_KINDS, fileKindFill } from "../../styles/dataviz";
import { nodeMenu } from "./storageActions";
import { effectiveChildren } from "./treemap";

/** Accessible list twin of the treemap: the focused folder's contents by size. */
export function FolderContents({
  node,
  hidden,
  onZoom,
}: {
  node: TreeNode | undefined;
  hidden: Set<string>;
  onZoom: (node: TreeNode) => void;
}) {
  const children = useMemo(() => (node ? (effectiveChildren(node, hidden) ?? []) : []), [node, hidden]);
  const total = children.reduce((s, c) => s + c.sizeBytes, 0);
  if (!node) return null;
  return (
    <section aria-label={`Contents of ${node.name}`} className="flex min-h-0 flex-col">
      <div className="flex items-baseline justify-between px-4 pb-2">
        <h3 className="text-[13px] font-medium text-fg">Largest items here</h3>
        <span className="num text-[12px] text-fg-muted">{formatBytes(node.sizeBytes, { base: 1000 })}</span>
      </div>
      <ul className="min-h-0 flex-1 overflow-y-auto px-2">
        {children.slice(0, 40).map((c) => {
          const share = total > 0 ? c.sizeBytes / total : 0;
          const zoomable = c.kind === "directory" && c.hasChildren;
          const Icon = c.kind === "directory" ? Folder : c.kind === "smallFiles" || c.kind === "remainder" ? Rows : File;
          return (
            <li key={c.id}>
              <button
                type="button"
                disabled={!zoomable && c.kind !== "file"}
                onClick={() => zoomable && onZoom(c)}
                onContextMenu={(e) => openContextMenu(e, nodeMenu(c, zoomable ? () => onZoom(c) : undefined), c.name)}
                className="flex w-full flex-col gap-1 rounded-[5px] px-2 py-1.5 text-left hover:bg-raised disabled:cursor-default disabled:hover:bg-transparent"
                title={c.path}
              >
                <div className="flex items-center gap-2 text-[12px]">
                  <Icon size={13} className="shrink-0 text-fg-subtle" />
                  <span className="min-w-0 flex-1 truncate text-fg">{c.kind === "smallFiles" ? "Small files" : c.name}</span>
                  <span className="num shrink-0 text-fg-muted">{formatBytes(c.sizeBytes, { base: 1000 })}</span>
                </div>
                <div className="relative ml-5 h-[3px] overflow-hidden rounded-full bg-[var(--viz-track)]">
                  <div
                    className="absolute inset-0 origin-left rounded-full"
                    style={{ transform: `scaleX(${share})`, background: c.kind === "directory" ? "var(--fg-subtle)" : fileKindFill(c.fileKind) }}
                  />
                </div>
              </button>
            </li>
          );
        })}
      </ul>
    </section>
  );
}

interface KindTotal {
  kind: FileKindGroup;
  bytes: number;
  count: number;
  extensions: ExtensionStat[];
}

export function kindTotals(stats: ExtensionStat[]): KindTotal[] {
  const map = new Map<FileKindGroup, KindTotal>();
  for (const s of stats) {
    const t = map.get(s.fileKind) ?? { kind: s.fileKind, bytes: 0, count: 0, extensions: [] };
    t.bytes += s.bytes;
    t.count += s.count;
    t.extensions.push(s);
    map.set(s.fileKind, t);
  }
  return [...map.values()]
    .map((t) => ({ ...t, extensions: t.extensions.sort((a, b) => b.bytes - a.bytes) }))
    .sort((a, b) => b.bytes - a.bytes || FILE_KIND_ORDER.indexOf(a.kind) - FILE_KIND_ORDER.indexOf(b.kind));
}

/** Legend and table twin for treemap colors: space by file type with top extensions. */
export function ByType({ stats }: { stats: ExtensionStat[] }) {
  const totals = useMemo(() => kindTotals(stats), [stats]);
  const all = totals.reduce((s, t) => s + t.bytes, 0);
  const max = totals[0]?.bytes ?? 0;
  return (
    <section aria-label="Space by file type" className="flex flex-col gap-2 px-4">
      <h3 className="text-[13px] font-medium text-fg">By type</h3>
      <table className="w-full text-[12px]">
        <tbody>
          {totals.map((t) => (
            <tr key={t.kind} className="align-middle" title={t.extensions.slice(0, 6).map((e) => `.${e.extension ?? "(none)"} ${formatBytes(e.bytes, { base: 1000 })}`).join("\n")}>
              <td className="w-3 py-1 pr-2">
                <span className="block size-2.5 rounded-[2px]" style={{ background: fileKindFill(t.kind) }} />
              </td>
              <td className="py-1 pr-2 text-fg">
                {FILE_KINDS[t.kind].label}
                <span className="ml-1.5 text-fg-subtle">
                  {t.extensions
                    .slice(0, 3)
                    .map((e) => (e.extension ? `.${e.extension}` : "no extension"))
                    .join(" ")}
                </span>
              </td>
              <td className="w-16 py-1 pr-2">
                <div className="relative h-[3px] overflow-hidden rounded-full bg-[var(--viz-track)]">
                  <div className="absolute inset-0 origin-left rounded-full bg-fg-subtle" style={{ transform: `scaleX(${max ? t.bytes / max : 0})` }} />
                </div>
              </td>
              <td className="num w-16 py-1 text-right text-fg">{formatBytes(t.bytes, { base: 1000 })}</td>
              <td className="num w-12 py-1 text-right text-fg-subtle">{formatPercent(all ? (t.bytes / all) * 100 : 0, 0)}</td>
            </tr>
          ))}
        </tbody>
      </table>
      <p className="text-[11px] text-fg-subtle">
        {formatCount(totals.reduce((s, t) => s + t.count, 0))} files. Hatched areas are small files grouped per folder.
      </p>
    </section>
  );
}
