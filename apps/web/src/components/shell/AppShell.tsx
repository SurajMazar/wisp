import { Outlet, useLocation } from "react-router";
import { ApprovalStack } from "../tools/ApprovalCard";
import { OnboardingFlow } from "../../features/onboarding/OnboardingFlow";
import { useSettingsStore } from "../../stores/settingsStore";
import { Sidebar } from "./Sidebar";

/** Chat surfaces own their full height (pinned composer, scrolling thread). */
const fullBleed = (path: string) => path === "/home" || path.startsWith("/conversations");

export function AppShell() {
  const { pathname } = useLocation();
  const onboarded = useSettingsStore((s) => s.settings?.onboarding.completed ?? true);
  if (!onboarded) return <OnboardingFlow />;
  return (
    <div className="relative flex h-full">
      <Sidebar />
      <ApprovalStack className="absolute right-4 top-10 z-[var(--z-popover)] w-[380px]" />
      {fullBleed(pathname) ? (
        <main className="flex min-w-0 flex-1 flex-col">
          <Outlet />
        </main>
      ) : (
        <main className="min-w-0 flex-1 overflow-y-auto">
          {/* Top inset leaves room for the transparent macOS title bar. */}
          <div className="mx-auto max-w-[760px] px-8 pb-12 pt-10">
            <Outlet />
          </div>
        </main>
      )}
    </div>
  );
}
