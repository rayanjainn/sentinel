import { ArrowsClockwise, Check, CheckCircle, Copy, Globe, ShieldCheck, WarningCircle, XCircle } from "@phosphor-icons/react";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import { useEffect, useState, type ReactNode } from "react";

import type { ErrorPayload } from "../../bindings/ErrorPayload";
import type { KeyStatus } from "../../bindings/KeyStatus";
import type { ProviderId } from "../../bindings/ProviderId";
import { Button, IconButton } from "../../components/Button";
import { Select } from "../../components/Field";
import { Skeleton } from "../../components/Skeleton";
import { ErrorState } from "../../components/States";
import { cx } from "../../lib/cx";
import { formatRelative } from "../../lib/format";
import { modelChoices, selectedModel } from "./model";
import { useAgent } from "./store";

const RECOMMENDED_LOCAL_MODEL = "llama3.1:8b";

function Row({ label, detail, children }: { label: string; detail?: ReactNode; children: ReactNode }) {
  return (
    <div className="flex items-start justify-between gap-6">
      <div className="flex min-w-0 flex-col gap-0.5">
        <span className="text-fg">{label}</span>
        {detail && <span className="text-[12px] text-fg-muted">{detail}</span>}
      </div>
      <div className="flex shrink-0 items-center gap-1.5">{children}</div>
    </div>
  );
}

function Subheading({ title, detail }: { title: string; detail: string }) {
  return (
    <div className="flex flex-col gap-0.5 pt-2">
      <h3 className="text-[13px] font-semibold text-fg">{title}</h3>
      <p className="text-[12px] text-fg-muted">{detail}</p>
    </div>
  );
}

function KeyStatusLine({ status }: { status: KeyStatus | undefined }) {
  if (!status) return <Skeleton className="h-4 w-28" />;
  switch (status.state) {
    case "valid":
      return (
        <span className="inline-flex items-center gap-1.5 text-[12px] text-signal">
          <CheckCircle size={13} weight="fill" /> Verified {formatRelative(status.checkedAtMs)}
        </span>
      );
    case "unverified":
      return (
        <span className="inline-flex items-center gap-1.5 text-[12px] text-warn" title={status.message}>
          <WarningCircle size={13} weight="fill" /> Saved, not verified yet
        </span>
      );
    case "invalid":
      return (
        <span className="inline-flex items-center gap-1.5 text-[12px] text-danger" title={status.message}>
          <XCircle size={13} weight="fill" /> Rejected by the provider
        </span>
      );
    case "missing":
      return <span className="text-[12px] text-fg-subtle">No key saved</span>;
    case "notRequired":
      return <span className="text-[12px] text-fg-subtle">No key needed</span>;
  }
}

function isError(value: KeyStatus | ErrorPayload): value is ErrorPayload {
  return "code" in value;
}

function KeyRow({ provider, name }: { provider: ProviderId; name: string }) {
  const status = useAgent((s) => s.statuses?.find((p) => p.id === provider)?.key);
  const setApiKey = useAgent((s) => s.setApiKey);
  const deleteApiKey = useAgent((s) => s.deleteApiKey);
  const [value, setValue] = useState("");
  const [busy, setBusy] = useState(false);
  const [problem, setProblem] = useState<string | ErrorPayload | null>(null);
  const stored = status?.state === "valid" || status?.state === "unverified";

  const check = async () => {
    setBusy(true);
    setProblem(null);
    const result = await setApiKey(provider, value);
    setBusy(false);
    if (isError(result)) setProblem(result);
    else if (result.state === "invalid") setProblem(`${name} rejected this key: ${result.message}`);
    else {
      setValue("");
      if (result.state === "unverified") setProblem(result.message);
    }
  };

  return (
    <div className="flex flex-col gap-2 border-t border-line pt-3 first:border-t-0 first:pt-0">
      <div className="flex items-center justify-between gap-3">
        <span className="text-fg">{name}</span>
        <KeyStatusLine status={status} />
      </div>
      <form
        className="flex items-center gap-1.5"
        onSubmit={(e) => {
          e.preventDefault();
          if (value.trim() && !busy) void check();
        }}
      >
        <input
          type="password"
          value={value}
          onChange={(e) => setValue(e.target.value)}
          aria-label={`${name} API key`}
          placeholder={stored ? "Paste a new key to replace the saved one" : "Paste API key"}
          autoComplete="off"
          spellCheck={false}
          className="selectable h-7 min-w-0 flex-1 rounded-[5px] border border-line bg-sunken px-2.5 text-[12px] text-fg placeholder:text-fg-subtle focus:border-line-strong focus:outline-none"
        />
        <Button type="submit" size="sm" variant="primary" disabled={!value.trim() || busy}>
          {busy ? "Checking key" : "Check and save"}
        </Button>
        {stored && (
          <Button
            size="sm"
            variant="dangerQuiet"
            disabled={busy}
            onClick={async () => {
              const failure = await deleteApiKey(provider);
              setProblem(failure);
            }}
          >
            Remove key
          </Button>
        )}
      </form>
      {typeof problem === "string" && <p className="selectable text-[12px] text-fg-muted">{problem}</p>}
      {problem !== null && typeof problem !== "string" && <ErrorState error={problem} compact />}
    </div>
  );
}

function CopyCommand({ command }: { command: string }) {
  const [copied, setCopied] = useState(false);
  return (
    <div className="flex items-center gap-2 rounded-[5px] border border-line bg-sunken py-1 pl-2.5 pr-1">
      <code className="num selectable flex-1 text-[12px] text-fg">{command}</code>
      <IconButton
        label={copied ? "Copied" : "Copy command"}
        size="sm"
        onClick={() => {
          void writeText(command).then(() => {
            setCopied(true);
            window.setTimeout(() => setCopied(false), 1600);
          });
        }}
      >
        {copied ? <Check size={13} weight="bold" className="text-signal" /> : <Copy size={13} />}
      </IconButton>
    </div>
  );
}

/** Keyed by the saved address, so it resets when settings change elsewhere. */
function AddressForm({ initial }: { initial: string }) {
  const settings = useAgent((s) => s.settings);
  const saveSettings = useAgent((s) => s.saveSettings);
  const refreshStatus = useAgent((s) => s.refreshStatus);
  const [url, setUrl] = useState(initial);
  const [urlError, setUrlError] = useState<ErrorPayload | null>(null);
  return (
    <>
      <form
        className="flex items-center gap-1.5"
        onSubmit={async (e) => {
          e.preventDefault();
          if (!settings) return;
          const failure = await saveSettings({ ...settings, ollamaBaseUrl: url });
          setUrlError(failure);
          if (!failure) void refreshStatus();
        }}
      >
        <label className="flex min-w-0 flex-1 items-center gap-2">
          <span className="shrink-0 text-[12px] text-fg-muted">Address</span>
          <input
            value={url}
            onChange={(e) => setUrl(e.target.value)}
            spellCheck={false}
            autoComplete="off"
            className="num selectable h-7 min-w-0 flex-1 rounded-[5px] border border-line bg-sunken px-2.5 text-[12px] text-fg focus:border-line-strong focus:outline-none"
          />
        </label>
        <Button type="submit" size="sm" disabled={url.trim() === initial}>
          Save address
        </Button>
      </form>
      {urlError && <ErrorState error={urlError} compact />}
    </>
  );
}

function OllamaSection() {
  const settings = useAgent((s) => s.settings);
  const ollama = useAgent((s) => s.ollama);
  const refreshStatus = useAgent((s) => s.refreshStatus);
  const [checking, setChecking] = useState(false);

  const recheck = async () => {
    setChecking(true);
    await refreshStatus();
    setChecking(false);
  };

  let status: ReactNode;
  if (!ollama) {
    status = <Skeleton className="h-4 w-48" />;
  } else if (ollama.running) {
    const tools = ollama.models.filter((m) => m.supportsTools).length;
    status = (
      <span className="inline-flex items-center gap-1.5 text-[12px] text-signal">
        <CheckCircle size={13} weight="fill" /> Running{ollama.version ? `, version ${ollama.version}` : ""}. {ollama.models.length}{" "}
        {ollama.models.length === 1 ? "model" : "models"} installed, {tools} with tool support.
      </span>
    );
  } else if (ollama.installed) {
    status = (
      <span className="inline-flex items-center gap-1.5 text-[12px] text-warn">
        <WarningCircle size={13} weight="fill" /> Installed but not running. Open the Ollama app or run ollama serve.
      </span>
    );
  } else {
    status = (
      <span className="inline-flex items-center gap-1.5 text-[12px] text-fg-muted">
        <WarningCircle size={13} weight="fill" /> Not installed. Download it from ollama.com/download, open it, then check again.
      </span>
    );
  }
  const needsModel = ollama?.running && !ollama.models.some((m) => m.id === RECOMMENDED_LOCAL_MODEL && m.supportsTools);

  return (
    <div className="flex flex-col gap-3">
      <Subheading title="Local models with Ollama" detail="Runs on this machine. Conversations and system data never leave it." />
      <div className="flex items-center justify-between gap-3">
        {status}
        <Button size="sm" variant="ghost" icon={<ArrowsClockwise size={13} />} disabled={checking} onClick={() => void recheck()}>
          {checking ? "Checking" : "Check again"}
        </Button>
      </div>
      {needsModel && (
        <div className="flex flex-col gap-1.5">
          <span className="text-[12px] text-fg-muted">Download the recommended model (about 4.9 GB) in a terminal:</span>
          <CopyCommand command={`ollama pull ${RECOMMENDED_LOCAL_MODEL}`} />
        </div>
      )}
      {settings && <AddressForm key={settings.ollamaBaseUrl} initial={settings.ollamaBaseUrl} />}
    </div>
  );
}

export function AgentSettingsSection() {
  const bootstrap = useAgent((s) => s.bootstrap);
  const settings = useAgent((s) => s.settings);
  const descriptors = useAgent((s) => s.descriptors);
  const ollama = useAgent((s) => s.ollama);
  const models = useAgent((s) => s.models);
  const modelErrors = useAgent((s) => s.modelErrors);
  const loadError = useAgent((s) => s.loadError);
  const selectProvider = useAgent((s) => s.selectProvider);
  const selectModel = useAgent((s) => s.selectModel);
  const loadModels = useAgent((s) => s.loadModels);
  const saveSettings = useAgent((s) => s.saveSettings);

  useEffect(() => {
    void bootstrap();
  }, [bootstrap]);

  if (loadError) return <ErrorState error={loadError} subject="Agent settings" onRetry={() => void bootstrap()} />;
  if (!settings || !descriptors) {
    return (
      <div className="flex flex-col gap-3">
        <Skeleton className="h-7 w-full" />
        <Skeleton className="h-7 w-full" />
        <Skeleton className="h-20 w-full" />
      </div>
    );
  }

  const provider = settings.activeProvider;
  const descriptor = descriptors.find((d) => d.id === provider);
  const model = selectedModel(settings, provider, descriptors);
  const choices = modelChoices(provider, model, models[provider], ollama?.running ? ollama.models : null, descriptor);
  const current = choices.find((m) => m.id === model);
  const modelError = modelErrors[provider];
  const cloudProviders = descriptors.filter((d) => d.requiresApiKey);

  return (
    <div className="flex flex-col gap-5">
      <Row
        label="Provider"
        detail={
          descriptor && (
            <span className={cx("inline-flex items-center gap-1.5", descriptor.kind === "local" && "text-signal")}>
              {descriptor.kind === "local" ? <ShieldCheck size={12} weight="bold" /> : <Globe size={12} weight="bold" />}
              {descriptor.dataNotice}
            </span>
          )
        }
      >
        <Select label="Provider" value={provider} onChange={(e) => void selectProvider(e.target.value as ProviderId)} className="w-52">
          {descriptors.map((d) => (
            <option key={d.id} value={d.id}>
              {d.displayName}
            </option>
          ))}
        </Select>
      </Row>

      <Row
        label="Model"
        detail={
          current && !current.supportsTools
            ? "This model can't call tools, so the agent can't read live data with it."
            : "Only models that support tool calling can use Sentinel's tools."
        }
      >
        <Select label="Model" value={model} onChange={(e) => void selectModel(provider, e.target.value)} className="w-52">
          {choices.map((m) => (
            <option key={m.id} value={m.id}>
              {m.supportsTools ? m.displayName : `${m.displayName} (no tool support)`}
            </option>
          ))}
        </Select>
        {provider !== "ollama" && (
          <IconButton label="Refresh model list" size="sm" onClick={() => void loadModels(provider)}>
            <ArrowsClockwise size={13} />
          </IconButton>
        )}
      </Row>
      {modelError && <ErrorState error={modelError} subject="The model list" compact />}

      <Row label="Tool steps per message" detail="How many rounds of reading data the agent may take before it must answer.">
        <input
          key={settings.maxToolRounds}
          type="number"
          min={1}
          max={32}
          defaultValue={settings.maxToolRounds}
          aria-label="Tool steps per message"
          onBlur={(e) => {
            const input = e.currentTarget;
            const next = Math.round(Number(input.value));
            if (Number.isFinite(next) && next !== settings.maxToolRounds) void saveSettings({ ...settings, maxToolRounds: next });
            else input.value = String(settings.maxToolRounds);
          }}
          className="num h-7 w-20 rounded-[5px] border border-line bg-sunken px-2.5 text-[12px] text-fg focus:border-line-strong focus:outline-none"
        />
      </Row>

      <OllamaSection />

      <div className="flex flex-col gap-3">
        <Subheading
          title="API keys"
          detail="One entry per provider in your system keychain. Each key is checked with its provider before it is saved and is never shown again."
        />
        {cloudProviders.map((d) => (
          <KeyRow key={d.id} provider={d.id} name={d.displayName} />
        ))}
      </div>
    </div>
  );
}
