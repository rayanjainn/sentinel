import {
  ClockCounterClockwise,
  Cpu,
  Gear,
  Globe,
  HardDrives,
  ListBullets,
  type Icon,
} from "@phosphor-icons/react";

import type { ViewId } from "../stores/settings";

export interface NavItem {
  id: ViewId;
  label: string;
  icon: Icon;
  group: "monitor" | "app";
}

export const NAV_ITEMS: NavItem[] = [
  { id: "processes", label: "Processes", icon: ListBullets, group: "monitor" },
  { id: "network", label: "Network", icon: Globe, group: "monitor" },
  { id: "resources", label: "CPU & Memory", icon: Cpu, group: "monitor" },
  { id: "storage", label: "Storage", icon: HardDrives, group: "monitor" },
  { id: "activity", label: "Activity", icon: ClockCounterClockwise, group: "app" },
  { id: "settings", label: "Settings", icon: Gear, group: "app" },
];
