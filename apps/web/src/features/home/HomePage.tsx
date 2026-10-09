import { useEffect } from "react";
import { Link, useNavigate } from "react-router";
import { Composer } from "../../components/chat/Composer";
import { useConnectionStore } from "../../stores/connectionStore";
import { useConversationStore } from "../../stores/conversationStore";

function greeting(date = new Date()): string {
  const h = date.getHours();
  if (h < 5) return "Good evening.";
  if (h < 12) return "Good morning.";
  if (h < 18) return "Good afternoon.";
  return "Good evening.";
}

export function HomePage() {
  const info = useConnectionStore((s) => s.info);
  const { summaries, loadList, send } = useConversationStore();
  const navigate = useNavigate();

  useEffect(() => {
    void loadList();
  }, [loadList]);

  return (
    <div className="pt-16">
      <h1 className="text-2xl font-semibold tracking-tight">{greeting()}</h1>
      <p className="mt-1 text-md text-text-muted">What would you like to do?</p>
      <div className="mt-6">
        <Composer
          autoFocus
          streaming={false}
          onSend={(t) => void send(t, {}).then((id) => id && navigate(`/conversations/${id}`))}
        />
      </div>

      {summaries.length > 0 && (
        <section className="mt-10">
          <h2 className="mb-2 text-sm font-semibold text-text-muted">Recent conversations</h2>
          <ul className="divide-y divide-border rounded-[var(--radius-md)] border border-border bg-surface">
            {summaries.slice(0, 5).map((s) => (
              <li key={s.id}>
                <Link to={`/conversations/${s.id}`} className="block px-3 py-2 hover:bg-surface-hover">
                  <span className="block truncate text-base">{s.title}</span>
                  {s.preview && <span className="block truncate text-sm text-text-muted">{s.preview}</span>}
                </Link>
              </li>
            ))}
          </ul>
        </section>
      )}

      {info && (
        <p className="mt-10 text-sm text-text-faint">Wisp engine {info.coreVersion} · data stored locally</p>
      )}
    </div>
  );
}
