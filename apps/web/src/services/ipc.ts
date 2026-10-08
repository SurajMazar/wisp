import type { IpcClient } from "@wisp/contracts";

let client: IpcClient | null = null;

/** The process-wide IPC client. Set once at startup (or per test). */
export function setIpcClient(c: IpcClient | null): void {
  client?.dispose();
  client = c;
}

export function ipc(): IpcClient {
  if (!client) throw new Error("IPC client not initialized");
  return client;
}

export function hasIpcClient(): boolean {
  return client !== null;
}
