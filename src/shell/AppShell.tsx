import { AnimatePresence, motion } from "motion/react";
import type { ComponentType } from "react";

import { EmptyState, StatePanel } from "../components/States";
import { AuditLogView } from "../features/agent";
import { ProcessesView } from "../features/processes/ProcessesView";
import { ResourcesView } from "../features/resources/ResourcesView";
import { SettingsView } from "../features/settings/SettingsView";
import type { ViewId } from "../stores/settings";
import { useSettings } from "../stores/settings";
import { AgentDock } from "./AgentDock";
import { Header } from "./Header";
import { Sidebar } from "./Sidebar";

function ActivityView() {
  return (
    <div className="h-full overflow-y-auto">
      <AuditLogView />
    </div>
  );
}

const VIEWS: Partial<Record<ViewId, ComponentType>> = {
  processes: ProcessesView,
  resources: ResourcesView,
  activity: ActivityView,
  settings: SettingsView,
};

function MissingView() {
  return (
    <StatePanel>
      <EmptyState title="This view is not part of this build" detail="Choose another view from the sidebar." />
    </StatePanel>
  );
}

export function AppShell() {
  const view = useSettings((s) => s.view);
  const View = VIEWS[view] ?? MissingView;
  return (
    <div className="flex h-full bg-ground">
      <Sidebar />
      <div className="flex min-w-0 flex-1 flex-col">
        <Header />
        <main className="relative min-h-0 flex-1">
          <AnimatePresence mode="popLayout" initial={false}>
            <motion.div
              key={view}
              className="absolute inset-0"
              initial={{ opacity: 0 }}
              animate={{ opacity: 1 }}
              exit={{ opacity: 0 }}
              transition={{ duration: 0.12 }}
            >
              <View />
            </motion.div>
          </AnimatePresence>
        </main>
      </div>
      <AgentDock />
    </div>
  );
}
