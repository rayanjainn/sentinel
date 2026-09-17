import { Desktop, Moon, Sun } from "@phosphor-icons/react";
import type { ReactNode } from "react";

import { Button } from "../../components/Button";
import { Segmented } from "../../components/Segmented";
import { AgentSettingsSection } from "../agent";
import { useSettings, type SamplingInterval, type ThemePreference } from "../../stores/settings";
import { PermissionList } from "../permissions/PermissionList";
import { NetworkSettings } from "./NetworkSettings";

export function SettingsSection({ id, title, description, children }: { id?: string; title: string; description?: string; children: ReactNode }) {
  return (
    <section id={id} className="grid grid-cols-[220px_minmax(0,1fr)] gap-10 border-t border-line py-7 first:border-t-0 first:pt-2">
      <div className="flex flex-col gap-1">
        <h2 className="text-[15px] font-semibold tracking-display text-fg">{title}</h2>
        {description && <p className="text-[12px] text-fg-muted">{description}</p>}
      </div>
      <div className="flex min-w-0 flex-col gap-5">{children}</div>
    </section>
  );
}

export function SettingRow({ label, detail, control }: { label: string; detail?: ReactNode; control: ReactNode }) {
  return (
    <div className="flex items-start justify-between gap-6">
      <div className="flex min-w-0 flex-col gap-0.5">
        <span className="text-fg">{label}</span>
        {detail && <span className="text-[12px] text-fg-muted">{detail}</span>}
      </div>
      <div className="shrink-0">{control}</div>
    </div>
  );
}

export function SettingsView() {
  const theme = useSettings((s) => s.theme);
  const setTheme = useSettings((s) => s.setTheme);
  const intervalMs = useSettings((s) => s.intervalMs);
  const setIntervalMs = useSettings((s) => s.setIntervalMs);
  const reopenOnboarding = useSettings((s) => s.reopenOnboarding);

  return (
    <div className="h-full overflow-y-auto">
      <div className="mx-auto max-w-[920px] px-8 py-6">
        <SettingsSection title="Appearance">
          <SettingRow
            label="Theme"
            detail="System follows your operating system’s light or dark setting."
            control={
              <Segmented<ThemePreference>
                label="Theme"
                value={theme}
                onChange={setTheme}
                options={[
                  { value: "dark", label: <><Moon size={13} /> Dark</> },
                  { value: "light", label: <><Sun size={13} /> Light</> },
                  { value: "system", label: <><Desktop size={13} /> System</> },
                ]}
              />
            }
          />
        </SettingsSection>

        <SettingsSection title="Sampling" description="How often live data is collected while a view needs it.">
          <SettingRow
            label="Update interval"
            detail="Faster updates cost a little more CPU. Hidden views are never sampled."
            control={
              <Segmented<`${SamplingInterval}`>
                label="Update interval"
                value={`${intervalMs}`}
                onChange={(v) => setIntervalMs(Number(v) as SamplingInterval)}
                options={[
                  { value: "500", label: "0.5 s" },
                  { value: "1000", label: "1 s" },
                  { value: "2000", label: "2 s" },
                ]}
              />
            }
          />
        </SettingsSection>

        <NetworkSettings />

        <SettingsSection title="Permissions" description="Optional access that makes storage scans and some actions complete.">
          <PermissionList />
          <div>
            <Button variant="ghost" size="sm" onClick={reopenOnboarding}>
              Show welcome screen
            </Button>
          </div>
        </SettingsSection>

        <SettingsSection title="AI agent">
          <AgentSettingsSection />
        </SettingsSection>
      </div>
    </div>
  );
}
