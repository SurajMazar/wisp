/**
 * Node-only helpers that run the real `wisp-core` binary. Used by integration tests that
 * exercise React → IPC → Rust → SQLite without the Swift host.
 */
import { spawn, type ChildProcessWithoutNullStreams } from "node:child_process";
import { existsSync } from "node:fs";
import { dirname, join } from "node:path";
import { createInterface } from "node:readline";
import { fileURLToPath } from "node:url";
import type { Envelope, Transport } from "@wisp/contracts";

const repoRoot = join(dirname(fileURLToPath(import.meta.url)), "../../..");

export function coreBinaryPath(): string {
  const p = process.env.WISP_CORE_BIN ?? join(repoRoot, "target/debug/wisp-core");
  if (!existsSync(p))
    throw new Error(`wisp-core binary not found at ${p} — run \`cargo build -p wisp-core\``);
  return p;
}

export interface CoreProcess {
  transport: Transport;
  /** Close stdin and wait for a clean exit. */
  stop(): Promise<number | null>;
  child: ChildProcessWithoutNullStreams;
}

export async function startCore(dataDir: string): Promise<CoreProcess> {
  const child = spawn(coreBinaryPath(), ["--data-dir", dataDir], { stdio: ["pipe", "pipe", "pipe"] });
  const receivers = new Set<(e: unknown) => void>();
  let resolveReady!: () => void;
  const ready = new Promise<void>((r) => (resolveReady = r));

  createInterface({ input: child.stdout }).on("line", (line) => {
    const msg = JSON.parse(line) as { kind?: string; event?: string };
    if (msg.kind === "event" && msg.event === "core.ready") resolveReady();
    for (const r of receivers) r(msg);
  });
  child.stderr.on("data", (d: Buffer) => process.stderr.write(d));

  const transport: Transport = {
    send(envelope: Envelope) {
      child.stdin.write(JSON.stringify(envelope) + "\n");
    },
    onReceive(h) {
      receivers.add(h);
      return () => receivers.delete(h);
    },
  };

  await ready;
  return {
    transport,
    child,
    stop: () =>
      new Promise((resolve) => {
        child.once("exit", (code) => resolve(code));
        child.stdin.end();
      }),
  };
}
