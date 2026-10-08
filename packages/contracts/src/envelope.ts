import { z } from "zod";
import { IpcError, PROTOCOL_VERSION } from "./generated/contracts";

/** Wire envelope. Mirrors `crates/ipc/src/envelope.rs`. */
export const Envelope = z.discriminatedUnion("kind", [
  z.strictObject({
    kind: z.literal("request"),
    v: z.literal(PROTOCOL_VERSION),
    id: z.string(),
    method: z.string(),
    params: z.unknown().optional(),
  }),
  z.strictObject({
    kind: z.literal("response"),
    v: z.literal(PROTOCOL_VERSION),
    id: z.string(),
    result: z.unknown().optional(),
    error: IpcError.optional(),
  }),
  z.strictObject({
    kind: z.literal("event"),
    v: z.literal(PROTOCOL_VERSION),
    event: z.string(),
    payload: z.unknown().optional(),
  }),
]);
export type Envelope = z.output<typeof Envelope>;
