import { describe, expect, it } from "vitest";
import { IpcClient, WispError, type Transport } from "../src";
import type { Envelope } from "../src/envelope";

function loopback(handler: (req: Extract<Envelope, { kind: "request" }>) => unknown) {
  let receive: (e: unknown) => void = () => {};
  const sent: Envelope[] = [];
  const transport: Transport = {
    send(env) {
      sent.push(env);
      if (env.kind === "request") queueMicrotask(() => receive(handler(env)));
    },
    onReceive(h) {
      receive = h;
      return () => {};
    },
  };
  return { transport, sent, push: (e: unknown) => receive(e) };
}

const settings = {
  general: { launchAtLogin: false, showMenuBarIcon: true },
  appearance: { theme: "dark", accent: "purple" },
  privacy: { mode: "localOnly" },
  onboarding: { completed: false },
};

describe("IpcClient", () => {
  it("sends validated requests and parses typed results", async () => {
    const lb = loopback((req) => ({ kind: "response", v: 1, id: req.id, result: settings }));
    const client = new IpcClient(lb.transport);
    const s = await client.call("settings.get");
    expect(s.appearance.theme).toBe("dark");
    expect(lb.sent[0]).toMatchObject({ kind: "request", method: "settings.get", params: {} });
  });

  it("rejects invalid params before sending", async () => {
    const lb = loopback(() => {
      throw new Error("must not be called");
    });
    const client = new IpcClient(lb.transport);
    await expect(
      client.call("settings.update", {
        // @ts-expect-error — invalid theme on purpose
        update: { section: "appearance", value: { theme: "neon" } },
      }),
    ).rejects.toMatchObject({ code: "invalidParams" });
    expect(lb.sent).toHaveLength(0);
  });

  it("surfaces host errors as WispError", async () => {
    const lb = loopback((req) => ({
      kind: "response",
      v: 1,
      id: req.id,
      error: { code: "unavailable", message: "Ollama isn't running.", details: "connect ECONNREFUSED" },
    }));
    const err = await new IpcClient(lb.transport).call("app.info").catch((e: unknown) => e);
    expect(err).toBeInstanceOf(WispError);
    expect(err).toMatchObject({ code: "unavailable", details: "connect ECONNREFUSED" });
  });

  it("rejects malformed results instead of trusting them", async () => {
    const lb = loopback((req) => ({
      kind: "response",
      v: 1,
      id: req.id,
      result: { appearance: { theme: 42 } },
    }));
    await expect(new IpcClient(lb.transport).call("settings.get")).rejects.toMatchObject({
      code: "internal",
    });
  });

  it("delivers typed events and ignores malformed ones", () => {
    const lb = loopback(() => null);
    const client = new IpcClient(lb.transport);
    const got: string[] = [];
    client.on("settings.changed", (s) => got.push(s.appearance.theme));
    lb.push({ kind: "event", v: 1, event: "settings.changed", payload: settings });
    lb.push({ kind: "event", v: 1, event: "settings.changed", payload: { appearance: { theme: 1 } } });
    lb.push({ kind: "event", v: 2, event: "settings.changed", payload: settings });
    expect(got).toEqual(["dark"]);
  });

  it("times out", async () => {
    const transport: Transport = { send() {}, onReceive: () => () => {} };
    await expect(new IpcClient(transport).call("app.info", {}, { timeoutMs: 10 })).rejects.toMatchObject({
      code: "timeout",
    });
  });
});
