import { WispMark } from "@wisp/icons";
import { cn } from "@wisp/ui";
import { NavLink } from "react-router";
import { sections } from "../../app/router/sections";
import { useConnectionStore } from "../../stores/connectionStore";
import { useSettingsStore } from "../../stores/settingsStore";

const privacyLabel = {
  localOnly: "Local only",
  localPreferred: "Local preferred",
  cloudAllowed: "Cloud allowed",
} as const;

export function Sidebar() {
  const connection = useConnectionStore((s) => s.status);
  const mode = useSettingsStore((s) => s.settings?.privacy.mode);

  return (
    <nav
      aria-label="Main"
      className="flex h-full w-[208px] shrink-0 flex-col border-r border-border bg-bg-sunken/70 px-2 pb-3 pt-10"
    >
      <div className="mb-4 flex items-center gap-2 px-2">
        <WispMark size={18} />
        <span className="text-md font-semibold tracking-tight">Wisp</span>
      </div>
      <ul className="flex flex-col gap-px">
        {sections.map(({ path, label, icon: Icon }) => (
          <li key={path}>
            <NavLink
              to={path}
              className={({ isActive }) =>
                cn(
                  "flex h-7 items-center gap-2 rounded-[var(--radius-sm)] px-2 text-base text-text-muted transition-colors duration-[var(--duration-fast)] hover:bg-surface-hover hover:text-text",
                  isActive && "bg-surface-hover text-text",
                )
              }
            >
              <Icon size={15} className="shrink-0" />
              {label}
            </NavLink>
          </li>
        ))}
      </ul>
      <div className="mt-auto space-y-1 px-2 text-sm text-text-faint">
        {mode && (
          <p className="flex items-center gap-1.5" title="Privacy mode">
            <span className="h-1.5 w-1.5 rounded-full bg-success" aria-hidden />
            {privacyLabel[mode]}
          </p>
        )}
        <p className="flex items-center gap-1.5">
          <span
            aria-hidden
            className={cn(
              "h-1.5 w-1.5 rounded-full",
              connection === "connected"
                ? "bg-success"
                : connection === "connecting"
                  ? "bg-warning"
                  : "bg-danger",
            )}
          />
          {connection === "connected"
            ? "Engine running"
            : connection === "connecting"
              ? "Starting engine…"
              : "Engine unavailable"}
        </p>
      </div>
    </nav>
  );
}
