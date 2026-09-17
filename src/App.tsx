import { MotionConfig } from "motion/react";
import { useEffect } from "react";

import { ContextMenuHost } from "./components/ContextMenu";
import { Toaster } from "./components/Toaster";
import { ActionDialog } from "./features/actions";
import { Onboarding } from "./features/onboarding/Onboarding";
import { PriorityDialog } from "./features/processes/PriorityDialog";
import { startSampling } from "./lib/sampling";
import { AppShell } from "./shell/AppShell";
import { HeaderSlotProvider } from "./shell/HeaderSlot";
import { useShortcuts } from "./shell/useShortcuts";
import { useThemeEffect } from "./shell/useTheme";
import { startPermissionWatch } from "./stores/permissions";
import { startResources } from "./stores/resources";
import { useSettings } from "./stores/settings";

export function App() {
  useThemeEffect();
  useShortcuts();
  const onboarded = useSettings((s) => s.onboarded);

  useEffect(() => {
    const stops = [startResources(), startPermissionWatch()];
    startSampling();
    return () => stops.forEach((stop) => stop());
  }, []);

  return (
    <MotionConfig reducedMotion="user">
      <HeaderSlotProvider>{onboarded ? <AppShell /> : <Onboarding />}</HeaderSlotProvider>
      <PriorityDialog />
      <ActionDialog />
      <ContextMenuHost />
      <Toaster />
    </MotionConfig>
  );
}
