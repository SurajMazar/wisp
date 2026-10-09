import { ipc } from "../services/ipc";

/** Copy via the native host (reliable inside the app), falling back to the web clipboard. */
export async function copyText(text: string): Promise<void> {
  try {
    await ipc().call("host.clipboard.writeText", { text });
  } catch {
    await navigator.clipboard?.writeText(text);
  }
}
