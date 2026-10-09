import { createHashRouter, Navigate } from "react-router";
import { AppShell } from "../../components/shell/AppShell";
import { AssistantPanel } from "../../features/assistant/AssistantPanel";
import { HomePage } from "../../features/home/HomePage";
import { ModelsPage } from "../../features/models/ModelsPage";
import { PermissionsPage } from "../../features/permissions/PermissionsPage";
import { UnavailablePage } from "../../features/placeholders/UnavailablePage";
import { SettingsPage } from "../../features/settings/SettingsPage";
import { sections } from "./sections";

const built: Record<string, React.ReactNode> = {
  "/home": <HomePage />,
  "/settings": <SettingsPage />,
  "/permissions": <PermissionsPage />,
  "/models": <ModelsPage />,
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
        { path: "*", element: <Navigate to="/home" replace /> },
      ],
    },
  ]);
}
