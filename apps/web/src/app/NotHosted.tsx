import { DisconnectedIcon } from "@wisp/icons";
import { EmptyState } from "@wisp/ui";

/** Shown when the UI is opened outside the Wisp app (e.g. a plain browser tab). */
export function NotHosted() {
  return (
    <div className="flex h-full items-center justify-center">
      <EmptyState icon={<DisconnectedIcon size={28} />} title="Wisp's engine isn't connected">
        This interface runs inside the Wisp app. Start it with <code className="font-mono">pnpm dev</code>,
        which opens the app pointed at this dev server.
      </EmptyState>
    </div>
  );
}
