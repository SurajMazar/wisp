import type { Transport } from "./client";
import type { Envelope } from "./envelope";

/**
 * Transport for the WKWebView hosted by the Swift app.
 * Outbound: `window.webkit.messageHandlers.wisp.postMessage(envelope)`.
 * Inbound:  the host calls `window.__wispReceive(envelope)` via `evaluateJavaScript`.
 */
interface WebKitBridge {
  webkit?: { messageHandlers?: { wisp?: { postMessage(message: unknown): void } } };
  __wispReceive?: (envelope: unknown) => void;
}

export function isHosted(): boolean {
  return typeof window !== "undefined" && !!(window as unknown as WebKitBridge).webkit?.messageHandlers?.wisp;
}

export function createWebKitTransport(): Transport {
  const w = window as unknown as WebKitBridge;
  const handlers = new Set<(e: unknown) => void>();
  w.__wispReceive = (envelope) => {
    for (const h of handlers) h(envelope);
  };
  return {
    send(envelope: Envelope) {
      const bridge = w.webkit?.messageHandlers?.wisp;
      if (!bridge) throw new Error("Wisp host bridge is unavailable");
      bridge.postMessage(envelope);
    },
    onReceive(handler) {
      handlers.add(handler);
      return () => handlers.delete(handler);
    },
  };
}
