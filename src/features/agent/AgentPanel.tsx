import { ArrowUp, Globe, NotePencil, ShieldCheck, Stop, Warning, X } from "@phosphor-icons/react";
import { AnimatePresence, motion } from "motion/react";
import { useEffect, useLayoutEffect, useRef, useState, type ReactNode } from "react";

import type { ProviderDescriptor } from "../../bindings/ProviderDescriptor";
import type { ProviderId } from "../../bindings/ProviderId";
import { Button, IconButton } from "../../components/Button";
import { Select } from "../../components/Field";
import { Skeleton } from "../../components/Skeleton";
import { ErrorState } from "../../components/States";
import { cx } from "../../lib/cx";
import { springInteraction, usePrefersReducedMotion } from "../../lib/motion";
import { useSettings } from "../../stores/settings";
import { Markdown } from "./Markdown";
import {
  EXAMPLE_REQUESTS,
  modelChoices,
  readiness,
  readinessFromError,
  selectedModel,
  type Readiness,
} from "./model";
import { PlanSection } from "./PlanSection";
import { useAgent } from "./store";
import { ToolCallRow } from "./ToolCallRow";
import type { ChatItem } from "./transcript";

export interface AgentPanelProps {
  onClose: () => void;
}

function DataBadge({ descriptor }: { descriptor: ProviderDescriptor | undefined }) {
  if (!descriptor) return <Skeleton className="h-4 w-44" />;
  const local = descriptor.kind === "local";
  return (
    <span
      title={descriptor.dataNotice}
      className={cx("inline-flex items-center gap-1.5 text-[12px]", local ? "text-signal" : "text-fg-muted")}
    >
      {local ? <ShieldCheck size={13} weight="bold" /> : <Globe size={13} weight="bold" />}
      {local ? "Local and private: nothing leaves this machine" : descriptor.dataNotice.replace("Requests are sent to", "Requests sent to")}
    </span>
  );
}

function openAgentSettings() {
  useSettings.getState().setView("settings");
}

function ReadinessNotice({ state, onCheck }: { state: Readiness; onCheck: () => void }) {
  const descriptors = useAgent((s) => s.descriptors);
  const name = (id: ProviderId) => descriptors?.find((d) => d.id === id)?.displayName ?? id;
  let title: string;
  let detail: ReactNode;
  let check = false;
  switch (state.state) {
    case "ready":
    case "checking":
      return null;
    case "missingKey":
      title = `Add an API key for ${name(state.provider)}`;
      detail = "It is checked with the provider, then stored in your system keychain.";
      break;
    case "invalidKey":
      title = `${name(state.provider)} rejected the saved API key`;
      detail = state.message;
      break;
    case "ollamaMissing":
      title = "Ollama is not installed";
      detail = "Install it from ollama.com/download and open it, or choose a cloud provider.";
      check = true;
      break;
    case "ollamaStopped":
      title = "Ollama is not running";
      detail = (
        <>
          Open the Ollama app or run <code className="num text-fg">ollama serve</code>. Sentinel looks for it at{" "}
          <span className="num">{state.baseUrl}</span>.
        </>
      );
      check = true;
      break;
    case "modelMissing":
      title = `${state.model} is not downloaded`;
      detail = (
        <>
          Run <code className="num text-fg">ollama pull {state.model}</code> in a terminal, or pick an installed model.
        </>
      );
      check = true;
      break;
    case "noTools":
      title = `${state.model} can't use tools`;
      detail = "The agent needs a model that can call tools to read live data. Pick one marked as supporting tools.";
      break;
  }
  return (
    <div role="note" className="flex gap-2.5 border-t border-line bg-sunken/60 px-4 py-3">
      <Warning size={15} weight="bold" className="mt-0.5 shrink-0 text-warn" />
      <div className="flex min-w-0 flex-col gap-1">
        <p className="font-medium text-fg">{title}</p>
        <p className="selectable text-[12px] text-fg-muted">{detail}</p>
        <div className="mt-1.5 flex gap-1.5">
          {check && (
            <Button size="sm" onClick={onCheck}>
              Check again
            </Button>
          )}
          <Button size="sm" variant="ghost" onClick={openAgentSettings}>
            Open agent settings
          </Button>
        </div>
      </div>
    </div>
  );
}

function EmptyChat({ onPick }: { onPick: (request: string) => void }) {
  return (
    <div className="flex flex-col gap-4 py-6">
      <div className="flex flex-col gap-1">
        <p className="text-[15px] font-semibold tracking-display text-fg">Ask about this machine</p>
        <p className="text-fg-muted">
          The agent reads live data first, then proposes changes as cards you approve one at a time. Nothing changes until you
          run them.
        </p>
      </div>
      <div className="flex flex-col divide-y divide-line overflow-hidden rounded-[8px] border border-line">
        {EXAMPLE_REQUESTS.map((request) => (
          <button
            key={request}
            type="button"
            onClick={() => onPick(request)}
            className="px-3 py-2.5 text-left text-[13px] text-fg-muted transition-colors duration-150 hover:bg-raised hover:text-fg"
          >
            {request}
          </button>
        ))}
      </div>
    </div>
  );
}

function Composer({
  draft,
  setDraft,
  running,
  onSend,
  onStop,
}: {
  draft: string;
  setDraft: (text: string) => void;
  running: boolean;
  onSend: () => void;
  onStop: () => void;
}) {
  const ref = useRef<HTMLTextAreaElement>(null);
  useLayoutEffect(() => {
    const el = ref.current;
    if (!el) return;
    el.style.height = "auto";
    el.style.height = `${Math.min(el.scrollHeight, 160)}px`;
  }, [draft]);
  return (
    <div className="border-t border-line p-3">
      <div className="flex items-end gap-2 rounded-[8px] border border-line bg-sunken py-1.5 pl-3 pr-1.5 focus-within:border-line-strong">
        <textarea
          ref={ref}
          rows={1}
          value={draft}
          aria-label="Message the agent"
          placeholder="Ask about processes, ports or storage"
          onChange={(e) => setDraft(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter" && !e.shiftKey && !e.nativeEvent.isComposing) {
              e.preventDefault();
              if (!running) onSend();
            }
          }}
          className="selectable my-0.5 min-h-5 flex-1 resize-none bg-transparent text-[13px] leading-5 text-fg placeholder:text-fg-subtle focus:outline-none"
        />
        {running ? (
          <Button size="sm" icon={<Stop size={11} weight="fill" />} onClick={onStop}>
            Stop
          </Button>
        ) : (
          <IconButton
            label="Send"
            size="sm"
            disabled={!draft.trim()}
            onClick={onSend}
            className="bg-signal text-signal-ink hover:bg-signal hover:text-signal-ink hover:brightness-110"
          >
            <ArrowUp size={13} weight="bold" />
          </IconButton>
        )}
      </div>
    </div>
  );
}

type Row = { key: string; item: ChatItem } | { key: string; tools: Extract<ChatItem, { kind: "tool" }>[] };

function groupRows(items: ChatItem[]): Row[] {
  const rows: Row[] = [];
  for (const item of items) {
    const last = rows[rows.length - 1];
    if (item.kind === "tool") {
      if (last && "tools" in last) last.tools.push(item);
      else rows.push({ key: `tools-${item.id}`, tools: [item] });
    } else {
      rows.push({ key: `${item.kind}-${item.id}`, item });
    }
  }
  return rows;
}

function ChatRow({ row }: { row: Row }) {
  if ("tools" in row) {
    return (
      <div className="flex flex-col gap-0.5">
        {row.tools.map((tool) => (
          <ToolCallRow key={tool.id} item={tool} />
        ))}
      </div>
    );
  }
  const { item } = row;
  switch (item.kind) {
    case "user":
      return (
        <div className="selectable ml-8 self-end whitespace-pre-wrap break-words rounded-[8px] bg-raised px-3 py-2 text-[13px] text-fg">
          {item.text}
        </div>
      );
    case "assistant":
      return <Markdown text={item.text} streaming={item.streaming} />;
    case "plan":
      return <PlanSection plan={item.plan} />;
    case "error":
      return <ErrorState error={item.error} subject="The agent" compact />;
    case "tool":
      return null;
  }
}

export function AgentPanel({ onClose }: AgentPanelProps) {
  const bootstrap = useAgent((s) => s.bootstrap);
  const settings = useAgent((s) => s.settings);
  const descriptors = useAgent((s) => s.descriptors);
  const statuses = useAgent((s) => s.statuses);
  const ollama = useAgent((s) => s.ollama);
  const models = useAgent((s) => s.models);
  const chat = useAgent((s) => s.chat);
  const loadError = useAgent((s) => s.loadError);
  const sendError = useAgent((s) => s.sendError);
  const send = useAgent((s) => s.send);
  const cancel = useAgent((s) => s.cancel);
  const newConversation = useAgent((s) => s.newConversation);
  const selectProvider = useAgent((s) => s.selectProvider);
  const selectModel = useAgent((s) => s.selectModel);
  const refreshStatus = useAgent((s) => s.refreshStatus);
  const dismissSendError = useAgent((s) => s.dismissSendError);
  const reduced = usePrefersReducedMotion();
  const [draft, setDraft] = useState("");
  const scrollRef = useRef<HTMLDivElement>(null);
  const stickToBottom = useRef(true);

  useEffect(() => {
    void bootstrap();
  }, [bootstrap]);

  useEffect(() => {
    const el = scrollRef.current;
    if (el && stickToBottom.current) el.scrollTop = el.scrollHeight;
  }, [chat.items]);

  const provider = settings?.activeProvider ?? "ollama";
  const descriptor = descriptors?.find((d) => d.id === provider);
  const model = settings ? selectedModel(settings, provider, descriptors) : "";
  const choices = modelChoices(provider, model, models[provider], ollama?.running ? ollama.models : null, descriptor);
  const ready = readiness({ settings, descriptors, statuses, ollama, models: models[provider] ?? null });
  const fromError = sendError ? readinessFromError(sendError, settings, model) : null;
  const notice = fromError ?? (ready.state === "ready" || ready.state === "checking" ? null : ready);

  const submit = async (text: string) => {
    setDraft("");
    stickToBottom.current = true;
    if (!(await send(text))) setDraft(text);
  };

  const rows = groupRows(chat.items);

  return (
    <div className="flex h-full min-h-0 flex-col">
      <header className="flex flex-col gap-2.5 border-b border-line px-4 pb-3 pt-3">
        <div className="flex items-center justify-between gap-2">
          <h2 className="text-[13px] font-semibold text-fg">Agent</h2>
          <div className="flex items-center gap-0.5">
            <IconButton label="New conversation" size="sm" onClick={() => void newConversation()}>
              <NotePencil size={14} />
            </IconButton>
            <IconButton label="Close agent panel" size="sm" onClick={onClose}>
              <X size={14} />
            </IconButton>
          </div>
        </div>
        {settings && descriptors ? (
          <div className="grid grid-cols-[minmax(0,2fr)_minmax(0,3fr)] gap-1.5">
            <Select
              label="Provider"
              value={provider}
              disabled={chat.running}
              onChange={(e) => void selectProvider(e.target.value as ProviderId)}
            >
              {descriptors.map((d) => (
                <option key={d.id} value={d.id}>
                  {d.displayName}
                </option>
              ))}
            </Select>
            <Select label="Model" value={model} disabled={chat.running} onChange={(e) => void selectModel(provider, e.target.value)}>
              {choices.map((m) => (
                <option key={m.id} value={m.id}>
                  {m.supportsTools ? m.displayName : `${m.displayName} (no tool support)`}
                </option>
              ))}
            </Select>
          </div>
        ) : (
          <div className="grid grid-cols-[2fr_3fr] gap-1.5">
            <Skeleton className="h-7" />
            <Skeleton className="h-7" />
          </div>
        )}
        <DataBadge descriptor={descriptor} />
      </header>

      <div
        ref={scrollRef}
        onScroll={(e) => {
          const el = e.currentTarget;
          stickToBottom.current = el.scrollHeight - el.scrollTop - el.clientHeight < 80;
        }}
        className="min-h-0 flex-1 overflow-y-auto px-4"
      >
        {loadError ? (
          <div className="py-6">
            <ErrorState error={loadError} subject="The agent" onRetry={() => void bootstrap()} />
          </div>
        ) : !settings ? (
          <div className="flex flex-col gap-2 py-6">
            <Skeleton className="h-4 w-2/3" />
            <Skeleton className="h-4 w-full" />
            <Skeleton className="h-4 w-5/6" />
          </div>
        ) : rows.length === 0 ? (
          <EmptyChat onPick={(request) => (ready.state === "ready" ? void submit(request) : setDraft(request))} />
        ) : (
          <div className="flex flex-col gap-3 py-4">
            <AnimatePresence initial={false}>
              {rows.map((row) => (
                <motion.div
                  key={row.key}
                  className="flex flex-col"
                  initial={reduced ? false : { opacity: 0, y: 4 }}
                  animate={{ opacity: 1, y: 0 }}
                  transition={springInteraction}
                >
                  <ChatRow row={row} />
                </motion.div>
              ))}
            </AnimatePresence>
            {sendError && !fromError && (
              <div className="flex flex-col gap-2">
                <ErrorState error={sendError} subject="Sending the message" compact />
                <div>
                  <Button size="sm" variant="ghost" onClick={dismissSendError}>
                    Dismiss
                  </Button>
                </div>
              </div>
            )}
          </div>
        )}
      </div>

      {notice && (
        <ReadinessNotice
          state={notice}
          onCheck={() => {
            dismissSendError();
            void refreshStatus();
          }}
        />
      )}
      <Composer
        draft={draft}
        setDraft={setDraft}
        running={chat.running}
        onSend={() => void submit(draft)}
        onStop={() => void cancel()}
      />
    </div>
  );
}
