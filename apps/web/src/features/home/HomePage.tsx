import { useConnectionStore } from "../../stores/connectionStore";

function greeting(date = new Date()): string {
  const h = date.getHours();
  if (h < 5) return "Good evening.";
  if (h < 12) return "Good morning.";
  if (h < 18) return "Good afternoon.";
  return "Good evening.";
}

export function HomePage() {
  const info = useConnectionStore((s) => s.info);
  return (
    <div className="pt-16">
      <h1 className="text-2xl font-semibold tracking-tight">{greeting()}</h1>
      <p className="mt-1 text-md text-text-muted">What would you like to do?</p>
      <div
        className="mt-6 rounded-[var(--radius-lg)] border border-border bg-surface px-4 py-3 text-text-faint"
        aria-disabled
      >
        Asking questions becomes available once an AI model is connected.
      </div>
      {info && (
        <p className="mt-10 text-sm text-text-faint">Wisp engine {info.coreVersion} · data stored locally</p>
      )}
    </div>
  );
}
