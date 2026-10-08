import { Outlet } from "react-router";
import { Sidebar } from "./Sidebar";

export function AppShell() {
  return (
    <div className="flex h-full">
      <Sidebar />
      <main className="min-w-0 flex-1 overflow-y-auto">
        {/* Top inset leaves room for the transparent macOS title bar. */}
        <div className="mx-auto max-w-[760px] px-8 pb-12 pt-10">
          <Outlet />
        </div>
      </main>
    </div>
  );
}
