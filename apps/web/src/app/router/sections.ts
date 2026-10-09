import {
  ActivityIcon,
  AgentsIcon,
  ConversationsIcon,
  FilesIcon,
  HomeIcon,
  MemoryIcon,
  ModelsIcon,
  PermissionsIcon,
  RoutinesIcon,
  SettingsIcon,
  SkillsIcon,
  VoiceIcon,
} from "@wisp/icons";
import type { ComponentType } from "react";

export interface Section {
  path: string;
  label: string;
  icon: ComponentType<{ size?: number; className?: string }>;
  /** Present while a section is not built yet: shown honestly instead of a fake screen. */
  unavailableNote?: string;
}

/** Sidebar order follows spec §71. */
export const sections: Section[] = [
  { path: "/home", label: "Home", icon: HomeIcon },
  {
    path: "/conversations",
    label: "Conversations",
    icon: ConversationsIcon,
  },
  {
    path: "/agents",
    label: "Agents",
    icon: AgentsIcon,
  },
  {
    path: "/skills",
    label: "Skills",
    icon: SkillsIcon,
  },
  {
    path: "/routines",
    label: "Routines",
    icon: RoutinesIcon,
  },
  {
    path: "/memory",
    label: "Memory",
    icon: MemoryIcon,
  },
  {
    path: "/files",
    label: "Files",
    icon: FilesIcon,
  },
  {
    path: "/models",
    label: "Models",
    icon: ModelsIcon,
  },
  {
    path: "/voice",
    label: "Voice",
    icon: VoiceIcon,
  },
  {
    path: "/activity",
    label: "Activity",
    icon: ActivityIcon,
  },
  {
    path: "/permissions",
    label: "Permissions",
    icon: PermissionsIcon,
  },
  { path: "/settings", label: "Settings", icon: SettingsIcon },
];
