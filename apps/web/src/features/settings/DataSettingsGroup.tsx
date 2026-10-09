import type { BackupList, ExportResult, UpdateInfo } from "@wisp/contracts";
import { WispError } from "@wisp/contracts";
import { Button, SettingsGroup, SettingsRow } from "@wisp/ui";
import { useEffect, useState } from "react";
import { ipc } from "../../services/ipc";

const when = (ms: number) =>
  new Date(ms).toLocaleString([], { month: "short", day: "numeric", hour: "numeric", minute: "2-digit" });

/** Backups, export and updates (spec §117). Nothing leaves the Mac unless you ask. */
export function DataSettingsGroup() {
  const [backups, setBackups] = useState<BackupList | null>(null);
  const [exported, setExported] = useState<ExportResult | null>(null);
  const [update, setUpdate] = useState<UpdateInfo | null>(null);
  const [note, setNote] = useState<string | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  useEffect(() => {
    void ipc()
      .call("backup.list")
      .then(setBackups)
      .catch(() => {});
  }, []);
  const run = async (what: string, f: () => Promise<void>) => {
    setBusy(what);
    setNote(null);
    try {
      await f();
    } catch (e) {
      setNote(e instanceof WispError ? e.message : "That didn't work.");
    } finally {
      setBusy(null);
    }
  };
  const latest = backups?.backups[0];

  return (
    <SettingsGroup
      title="Your data"
      description="Everything stays on this Mac. Backups happen automatically once a day; the last 7 are kept."
    >
      <SettingsRow
        label="Backups"
        hint={
          latest
            ? `Last backup ${when(latest.createdAtMs)} · ${backups?.backups.length} kept`
            : "No backup yet"
        }
        control={
          <Button
            size="sm"
            disabled={busy !== null}
            onClick={() =>
              void run("backup", async () => {
                await ipc().call("backup.create");
                setBackups(await ipc().call("backup.list"));
              })
            }
          >
            {busy === "backup" ? "Backing up…" : "Back up now"}
          </Button>
        }
      />
      <SettingsRow
        label="Export my data"
        hint={
          exported
            ? `Saved ${exported.conversations} conversations, ${exported.memories} memories, ${exported.skills} skills, ${exported.agents} agents, ${exported.routines} routines to ${exported.path}`
            : "Conversations, memories, skills, agents, routines and settings as JSON."
        }
        control={
          <Button
            size="sm"
            disabled={busy !== null}
            onClick={() => void run("export", async () => setExported(await ipc().call("data.export")))}
          >
            {busy === "export" ? "Exporting…" : "Export"}
          </Button>
        }
      />
      <SettingsRow
        label="Updates"
        hint={
          update
            ? update.updateAvailable
              ? `Wisp ${update.latest} is available (you have ${update.current}).`
              : `You're up to date (${update.current}).`
            : "Checks GitHub only when you ask. Nothing is installed automatically."
        }
        control={
          update?.updateAvailable && update.url ? (
            <a
              href={update.url}
              target="_blank"
              rel="noreferrer"
              className="text-sm text-accent hover:underline"
            >
              Download {update.latest}
            </a>
          ) : (
            <Button
              size="sm"
              disabled={busy !== null}
              onClick={() => void run("update", async () => setUpdate(await ipc().call("update.check")))}
            >
              {busy === "update" ? "Checking…" : "Check for updates"}
            </Button>
          )
        }
      />
      {note && (
        <p role="alert" className="px-4 pb-3 text-sm text-danger">
          {note}
        </p>
      )}
    </SettingsGroup>
  );
}
