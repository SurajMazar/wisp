import { Outlet, useLocation } from "react-router";
import { Sidebar } from "./Sidebar";

/** Chat surfaces own their full height (pinned composer, scrolling thread). */
const fullBleed = (path: string) => path === "/" || path.startsWith("/conversations");

export function AppShell() {
  const { pathname } = useLocation();
  return (
    <div className="flex h-full">
      <Sidebar />
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
