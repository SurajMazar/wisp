/**
 * Runs spoken-style background tasks through a real `wisp-core` with your local model, with a
 * stand-in host that records (and approves) every app action instead of performing it.
 *
 *   pnpm tsx scripts/validation/agent-check.ts [--general qwen3-vl:4b-instruct]
 */
import { spawn, execFileSync } from "node:child_process";
import { mkdtempSync } from "node:fs";
import { homedir, tmpdir } from "node:os";
import { join } from "node:path";
import { createInterface } from "node:readline";
import { root } from "./harness";

const arg = (k: string) => {
  const i = process.argv.indexOf(k);
  return i > -1 ? process.argv[i + 1] : undefined;
};
type Env = {
  kind: string;
  id?: string;
  event?: string;
  method?: string;
  params?: Record<string, unknown>;
  payload?: Record<string, unknown>;
  result?: unknown;
  error?: { message: string };
};

const tasks: string[] = [
  "remind me tomorrow at 9 to call Sam",
  "what's on my calendar today?",
  "pause the music",
  "remind me to buy milk",
  "what's on my desktop?",
  "skip this song",
];

async function main() {
  execFileSync("cargo", ["build", "-q", "--release", "-p", "wisp-core"], { cwd: root, stdio: "inherit" });
  const core = spawn(
    join(root, "target/release/wisp-core"),
    ["--data-dir", mkdtempSync(join(tmpdir(), "wisp-agent-"))],
    {
      stdio: ["pipe", "pipe", "ignore"],
    },
  );
  const write = (e: unknown) => core.stdin!.write(JSON.stringify(e) + "\n");
  const waiting = new Map<string, (e: Env) => void>();
  const listeners: ((e: Env) => void)[] = [];
  const performed: unknown[] = [];
  const ready = new Promise<void>((resolve) =>
    createInterface({ input: core.stdout! }).on("line", (line) => {
      const e = JSON.parse(line) as Env;
      if (e.kind === "event" && e.event === "core.ready") resolve();
      if (e.kind === "response" && e.id && waiting.has(e.id)) {
        waiting.get(e.id)!(e);
        waiting.delete(e.id);
      }
      if (e.kind === "request" && e.id) {
        // Stand-in host: record app actions, pretend they worked; everything else is unavailable.
        if (e.method === "host.apps.perform") {
          performed.push(e.params?.call);
          const call = e.params?.call as { tool: string };
          const output =
            call.tool === "listEvents" ? "09:30–10:00: Standup\n14:00–15:00: Design review" : undefined;
          write({
            v: 1,
            kind: "response",
            id: e.id,
            result: { output, verification: { status: "verified", detail: "done" } },
          });
        } else if (e.method === "host.notify") {
          write({ v: 1, kind: "response", id: e.id, result: {} });
        } else {
          write({ v: 1, kind: "response", id: e.id, error: { code: "unavailable", message: "no host" } });
        }
      }
      // Approve whatever is asked, as the user would.
      if (e.kind === "event" && e.event === "tool.approvalRequested")
        write({
          v: 1,
          kind: "request",
          id: `ok:${Math.random()}`,
          method: "tool.respond",
          params: { id: e.payload?.id, decision: "allowOnce" },
        });
      for (const l of listeners) l(e);
    }),
  );
  let n = 0;
  const call = <T>(method: string, params?: unknown) =>
    new Promise<T>((resolve, reject) => {
      const id = `ag:${++n}`;
      waiting.set(id, (e) => (e.error ? reject(new Error(e.error.message)) : resolve(e.result as T)));
      write({ v: 1, kind: "request", id, method, params: params ?? null });
    });
  await ready;
  const models = JSON.parse(
    execFileSync("sqlite3", [
      join(homedir(), "Library/Application Support/Wisp/database/wisp.sqlite3"),
      "select value from settings where key='models'",
    ]).toString(),
  );
  const general = arg("--general");
  if (general) models.general = models.vision = { providerId: "ollama", model: general };
  await call("settings.update", { update: { section: "models", value: models } });
  if (process.argv.includes("--mcp")) {
    // Connect the test MCP server and give tasks that need it.
    await call("settings.update", { update: { section: "privacy", value: { mode: "cloudAllowed" } } });
    await call("settings.update", {
      update: {
        section: "connections",
        value: {
          servers: [
            {
              id: "notes",
              name: "Notes service",
              enabled: true,
              localOnly: true,
              tools: [],
              transport: {
                kind: "stdio",
                config: {
                  command: "/opt/homebrew/bin/node",
                  args: [join(root, "crates/mcp/tests/fixtures/test-server.mjs")],
                  env: [],
                },
              },
            },
          ],
        },
      },
    });
    for (let i = 0; i < 50; i++) {
      const s = await call<{ servers: { state: string }[] }>("mcp.status");
      if (s.servers[0]?.state === "ready") break;
      await new Promise((r) => setTimeout(r, 100));
    }
    tasks.splice(
      0,
      tasks.length,
      "save a note called Groceries with milk and eggs in my notes service",
      "echo hello world using the notes service",
    );
  }

  const repeat = Number(arg("--repeat") ?? 1);
  const only = arg("--only");
  const chosen = only ? tasks.filter((t) => t.includes(only)) : tasks;
  for (const goal of chosen.flatMap((t) => Array<string>(repeat).fill(t))) {
    performed.length = 0;
    const t0 = Date.now();
    const done = new Promise<Record<string, unknown>>((resolve) =>
      listeners.push((e) => {
        const s = e.payload?.status as string;
        if (
          e.event === "agent.runUpdated" &&
          ["completed", "failed", "cancelled", "stuck", "needsInput"].includes(s)
        )
          resolve(e.payload!);
      }),
    );
    await call("agent.run", { goal, background: true });
    const run = await done;
    listeners.length = 0;
    console.log(`\n“${goal}” → ${run.status} in ${((Date.now() - t0) / 1000).toFixed(1)} s`);
    for (const p of performed) console.log(`   app action: ${JSON.stringify(p)}`);
    for (const st of (run.steps as { kind: string; title: string; detail?: string }[]).filter(
      (x) => x.kind === "act",
    ))
      console.log(`   step: ${st.title}${st.detail ? ` (${st.detail})` : ""}`);
    console.log(`   says: ${run.note}`);
  }
  core.stdin!.end();
}

main().catch((e) => {
  console.error(e);
  process.exit(1);
});
