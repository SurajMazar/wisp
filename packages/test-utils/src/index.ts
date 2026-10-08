import {
  PROTOCOL_VERSION,
  type Envelope,
  type MethodName,
  type MethodParams,
  type MethodResult,
  type Transport,
  type EventName,
  type EventPayload,
} from "@wisp/contracts";

type Handler<M extends MethodName> = (params: MethodParams<M>) => MethodResult<M> | Promise<MethodResult<M>>;

/**
 * In-memory transport for UI tests. Register typed handlers per method; emit typed events.
 * Unhandled methods respond with `methodNotFound`, exactly like the real core.
 */
export class MockTransport implements Transport {
  readonly sent: Envelope[] = [];
  private readonly handlers = new Map<string, (p: unknown) => unknown>();
  private readonly receivers = new Set<(e: unknown) => void>();

  handle<M extends MethodName>(method: M, handler: Handler<M>): this {
    this.handlers.set(method, handler as (p: unknown) => unknown);
    return this;
  }

  emit<E extends EventName>(event: E, payload: EventPayload<E>): void {
    this.deliver({ kind: "event", v: PROTOCOL_VERSION, event, payload });
  }

  send(envelope: Envelope): void {
    this.sent.push(envelope);
    if (envelope.kind !== "request") return;
    const handler = this.handlers.get(envelope.method);
    void (async () => {
      if (!handler) {
        this.deliver({
          kind: "response",
          v: PROTOCOL_VERSION,
          id: envelope.id,
          error: {
            code: "methodNotFound",
            message: "That action isn't supported.",
            details: envelope.method,
          },
        });
        return;
      }
      try {
        const result = await handler(envelope.params);
        this.deliver({ kind: "response", v: PROTOCOL_VERSION, id: envelope.id, result });
      } catch (e) {
        this.deliver({
          kind: "response",
          v: PROTOCOL_VERSION,
          id: envelope.id,
          error: { code: "internal", message: "Mock handler failed.", details: String(e) },
        });
      }
    })();
  }

  onReceive(handler: (e: unknown) => void): () => void {
    this.receivers.add(handler);
    return () => this.receivers.delete(handler);
  }

  private deliver(e: unknown) {
    queueMicrotask(() => {
      for (const r of this.receivers) r(e);
    });
  }
}
