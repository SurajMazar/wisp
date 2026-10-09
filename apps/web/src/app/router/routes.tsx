import { Spinner } from "@wisp/ui";
import { lazy, Suspense, type ComponentType, type LazyExoticComponent } from "react";
import { createHashRouter, Navigate } from "react-router";
import { AppShell } from "../../components/shell/AppShell";
import { AssistantPanel } from "../../features/assistant/AssistantPanel";
import { ConversationsPage } from "../../features/conversations/ConversationsPage";
import { HomePage } from "../../features/home/HomePage";
import { UnavailablePage } from "../../features/placeholders/UnavailablePage";
import { sections } from "./sections";

/** Secondary pages load on first visit; chat (Home, Conversations, the panel) is always ready. */
type Page = ComponentType<object>;
const lazyPage = <K extends string>(load: () => Promise<Record<K, Page>>, name: K) =>
  lazy(async () => ({ default: (await load())[name] }));

const SettingsPage = lazyPage(() => import("../../features/settings/SettingsPage"), "SettingsPage");
const PermissionsPage = lazyPage(
  () => import("../../features/permissions/PermissionsPage"),
  "PermissionsPage",
);
const ModelsPage = lazyPage(() => import("../../features/models/ModelsPage"), "ModelsPage");
const VoicePage = lazyPage(() => import("../../features/voice/VoicePage"), "VoicePage");
const ActivityPage = lazyPage(() => import("../../features/activity/ActivityPage"), "ActivityPage");
const AgentsPage = lazyPage(() => import("../../features/agents/AgentsPage"), "AgentsPage");
const SkillsPage = lazyPage(() => import("../../features/skills/SkillsPage"), "SkillsPage");
const MemoryPage = lazyPage(() => import("../../features/memory/MemoryPage"), "MemoryPage");
const FilesPage = lazyPage(() => import("../../features/files/FilesPage"), "FilesPage");
const RoutinesPage = lazyPage(() => import("../../features/routines/RoutinesPage"), "RoutinesPage");

function Lazy({ page: P }: { page: LazyExoticComponent<Page> }) {
  return (
    <Suspense
      fallback={
        <div className="flex items-center gap-2 pt-16 text-text-muted">
          <Spinner /> Loading…
        </div>
      }
    >
      <P />
    </Suspense>
  );
}

const built: Record<string, React.ReactNode> = {
  "/home": <HomePage />,
  "/settings": <Lazy page={SettingsPage} />,
  "/permissions": <Lazy page={PermissionsPage} />,
  "/models": <Lazy page={ModelsPage} />,
  "/conversations": <ConversationsPage />,
  "/voice": <Lazy page={VoicePage} />,
  "/activity": <Lazy page={ActivityPage} />,
  "/agents": <Lazy page={AgentsPage} />,
  "/skills": <Lazy page={SkillsPage} />,
  "/memory": <Lazy page={MemoryPage} />,
  "/files": <Lazy page={FilesPage} />,
  "/routines": <Lazy page={RoutinesPage} />,
};

/** Hash routing: works identically under wisp:// in the app and the Vite dev server. */
export function createRouter() {
  return createHashRouter([
    // Floating assistant panel: its own surface, no app chrome.
    { path: "/assistant", element: <AssistantPanel /> },
    {
      path: "/",
      element: <AppShell />,
      children: [
        { index: true, element: <Navigate to="/home" replace /> },
        ...sections.map((s) => ({
          path: s.path.slice(1),
          element: built[s.path] ?? <UnavailablePage section={s} />,
        })),
        // A specific conversation (linked from Home, history and the panel).
        { path: "conversations/:id", element: <ConversationsPage /> },
        { path: "*", element: <Navigate to="/home" replace /> },
      ],
    },
  ]);
}
