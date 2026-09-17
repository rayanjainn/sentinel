import { AnimatePresence, motion } from "motion/react";
import { lazy, Suspense, type ComponentType } from "react";

import { SkeletonRows } from "../components/Skeleton";
import { ViewBoundary } from "../components/ViewBoundary";
import { AuditLogView } from "../features/agent";
import { SettingsView } from "../features/settings/SettingsView";
import type { ViewId } from "../stores/settings";
import { useSettings } from "../stores/settings";
import { AgentDock } from "./AgentDock";
import { Header } from "./Header";
import { NAV_ITEMS } from "./navigation";
import { Sidebar } from "./Sidebar";

function ActivityView() {
  return (
    <div className="h-full overflow-y-auto">
      <AuditLogView />
    </div>
  );
}

// Monitoring views load on first visit so the map, charts and treemap code stay out of startup.
const VIEWS: Record<ViewId, ComponentType> = {
  processes: lazy(() => import("../features/processes/ProcessesView").then((m) => ({ default: m.ProcessesView }))),
  network: lazy(() => import("../features/network/NetworkView").then((m) => ({ default: m.NetworkView }))),
  resources: lazy(() => import("../features/resources/ResourcesView").then((m) => ({ default: m.ResourcesView }))),
  storage: lazy(() => import("../features/storage/StorageView").then((m) => ({ default: m.StorageView }))),
  activity: ActivityView,
  settings: SettingsView,
};

function ViewFallback() {
  return (
    <div className="h-full overflow-hidden">
      <div className="h-[30px] border-b border-line" />
      <SkeletonRows rows={16} columns={[5, 2, 2, 2, 2, 3]} />
    </div>
  );
}

export function AppShell() {
  const view = useSettings((s) => s.view);
  const View = VIEWS[view];
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
              <ViewBoundary key={view} name={NAV_ITEMS.find((n) => n.id === view)?.label ?? "This view"}>
                <Suspense fallback={<ViewFallback />}>
                  <View />
                </Suspense>
              </ViewBoundary>
            </motion.div>
          </AnimatePresence>
        </main>
      </div>
      <AgentDock />
    </div>
  );
}
