import type { z } from "zod";
import {
  events,
  methods,
  PROTOCOL_VERSION,
  type EventName,
  type EventPayload,
  type IpcError,
  type MethodName,
  type MethodParams,
  type MethodResult,
} from "./generated/contracts";
import { Envelope } from "./envelope";

/** Moves envelopes between the UI and the host. Implementations: WebKit bridge, test mocks. */
export interface Transport {
  send(envelope: Envelope): void;
  /** Subscribe to inbound envelopes. Returns an unsubscribe function. */
  onReceive(handler: (envelope: unknown) => void): () => void;
}

/** Error thrown by {@link IpcClient.call}. `message` is user-safe; `details` is technical. */
export class WispError extends Error {
  readonly code: IpcError["code"];
  readonly details: string | undefined;

  constructor(error: IpcError) {
    super(error.message);
    this.name = "WispError";
    this.code = error.code;
    this.details = error.details ?? undefined;
  }
}

type Listener<E extends EventName> = (payload: EventPayload<E>) => void;

export interface CallOptions {
  timeoutMs?: number;
  signal?: AbortSignal;
}

/**
 * Typed IPC client. Every outbound param and inbound result/event is validated with the
 * generated Zod schemas; malformed messages are rejected rather than trusted.
 */
export class IpcClient {
  private nextId = 1;
  private readonly pending = new Map<
    string,
    {
      method: MethodName;
      resolve: (v: unknown) => void;
      reject: (e: unknown) => void;
      timer?: ReturnType<typeof setTimeout>;
    }
  >();
  private readonly listeners = new Map<string, Set<(p: unknown) => void>>();
  private readonly unsubscribe: () => void;

  constructor(private readonly transport: Transport) {
    this.unsubscribe = transport.onReceive((raw) => this.receive(raw));
  }

  dispose(): void {
    this.unsubscribe();
    for (const [, p] of this.pending) {
      if (p.timer) clearTimeout(p.timer);
      p.reject(new WispError({ code: "cancelled", message: "The connection closed." }));
    }
    this.pending.clear();
  }

  call<M extends MethodName>(
    method: M,
    ...[params, options]: MethodParams<M> extends Record<string, never>
      ? [params?: MethodParams<M>, options?: CallOptions]
      : [params: MethodParams<M>, options?: CallOptions]
  ): Promise<MethodResult<M>> {
    const spec = methods[method];
    const parsed = (spec.params as z.ZodType).safeParse(params ?? {});
    if (!parsed.success) {
      return Promise.reject(
        new WispError({
          code: "invalidParams",
          message: "The request was malformed.",
          details: parsed.error.message,
        }),
      );
    }
    const id = `u${this.nextId++}`;
    return new Promise<MethodResult<M>>((resolve, reject) => {
      const entry: {
        method: MethodName;
        resolve: (v: unknown) => void;
        reject: (e: unknown) => void;
        timer?: ReturnType<typeof setTimeout>;
      } = {
        method,
        resolve: resolve as (v: unknown) => void,
        reject,
      };
      const timeoutMs = options?.timeoutMs ?? 30_000;
      entry.timer = setTimeout(() => {
        this.pending.delete(id);
        reject(
          new WispError({ code: "timeout", message: "Wisp took too long to respond.", details: method }),
        );
      }, timeoutMs);
      options?.signal?.addEventListener("abort", () => {
        if (this.pending.delete(id)) {
          clearTimeout(entry.timer);
          reject(new WispError({ code: "cancelled", message: "Cancelled." }));
        }
      });
      this.pending.set(id, entry);
      this.transport.send({ kind: "request", v: PROTOCOL_VERSION, id, method, params: parsed.data });
    });
  }

  on<E extends EventName>(event: E, listener: Listener<E>): () => void {
    let set = this.listeners.get(event);
    if (!set) this.listeners.set(event, (set = new Set()));
    set.add(listener as (p: unknown) => void);
    return () => set.delete(listener as (p: unknown) => void);
  }

  private receive(raw: unknown): void {
    const env = Envelope.safeParse(raw);
    if (!env.success) {
      console.warn("wisp: dropped malformed envelope", env.error.message);
      return;
    }
    const msg = env.data;
    if (msg.kind === "response") {
      const p = this.pending.get(msg.id);
      if (!p) return;
      this.pending.delete(msg.id);
      if (p.timer) clearTimeout(p.timer);
      if (msg.error) {
        p.reject(new WispError(msg.error));
        return;
      }
      const result = (methods[p.method].result as z.ZodType).safeParse(msg.result);
      if (result.success) p.resolve(result.data);
      else
        p.reject(
          new WispError({
            code: "internal",
            message: "Wisp sent an unexpected response.",
            details: result.error.message,
          }),
        );
    } else if (msg.kind === "event") {
      const schema = (events as Record<string, z.ZodType | undefined>)[msg.event];
      const set = this.listeners.get(msg.event);
      if (!schema || !set?.size) return;
      const payload = schema.safeParse(msg.payload);
      if (!payload.success) {
        console.warn(`wisp: dropped malformed ${msg.event} event`, payload.error.message);
        return;
      }
      for (const l of set) l(payload.data);
    }
  }
}
