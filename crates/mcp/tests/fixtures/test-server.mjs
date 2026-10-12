// A minimal MCP server for Wisp's tests (no dependencies).
//   node test-server.mjs                      stdio
//   node test-server.mjs --http 0 [--token t] Streamable HTTP; prints "PORT <n>" when listening
import http from "node:http";
import process from "node:process";
import readline from "node:readline";

const tools = [
  {
    name: "echo",
    title: "Echo",
    description: "Repeat the text back.",
    inputSchema: { type: "object", properties: { text: { type: "string" } }, required: ["text"] },
    annotations: { readOnlyHint: true },
  },
  {
    name: "add_note",
    description: "Save a note to the notebook.",
    inputSchema: {
      type: "object",
      properties: { title: { type: "string" }, body: { type: "string" } },
      required: ["title"],
    },
  },
  {
    name: "wipe",
    description: "Delete every note.",
    inputSchema: { type: "object", properties: {} },
    annotations: { destructiveHint: true },
  },
  { name: "fail", description: "Always fails.", inputSchema: { type: "object" } },
];

function handle(msg) {
  if (msg.id === undefined) return null; // notification
  const ok = (result) => ({ jsonrpc: "2.0", id: msg.id, result });
  switch (msg.method) {
    case "initialize":
      return ok({
        protocolVersion: "2025-06-18",
        capabilities: { tools: {} },
        serverInfo: { name: "wisp-test-server", version: "1.0.0" },
      });
    case "tools/list":
      // Two pages, to exercise cursors.
      return msg.params?.cursor === "page2"
        ? ok({ tools: tools.slice(2) })
        : ok({ tools: tools.slice(0, 2), nextCursor: "page2" });
    case "tools/call": {
      const { name, arguments: a = {} } = msg.params ?? {};
      if (name === "echo") return ok({ content: [{ type: "text", text: `echo: ${a.text}` }] });
      if (name === "add_note")
        return ok({
          content: [{ type: "text", text: `Saved “${a.title}” (${(a.body ?? "").length} characters)` }],
        });
      if (name === "wipe") return ok({ content: [{ type: "text", text: "Wiped." }] });
      if (name === "fail")
        return ok({ content: [{ type: "text", text: "Something went wrong" }], isError: true });
      return { jsonrpc: "2.0", id: msg.id, error: { code: -32602, message: `Unknown tool ${name}` } };
    }
    default:
      return { jsonrpc: "2.0", id: msg.id, error: { code: -32601, message: "Method not found" } };
  }
}

const httpIdx = process.argv.indexOf("--http");
if (httpIdx === -1) {
  const rl = readline.createInterface({ input: process.stdin });
  let pinged = false;
  rl.on("line", (line) => {
    if (!line.trim()) return;
    const msg = JSON.parse(line);
    if (msg.method === undefined) return; // a reply to our ping
    if (msg.method === "tools/list" && !pinged) {
      pinged = true;
      process.stdout.write(JSON.stringify({ jsonrpc: "2.0", id: "srv-1", method: "ping" }) + "\n");
    }
    process.stderr.write(`handling ${msg.method}\n`);
    const reply = handle(msg);
    if (reply) process.stdout.write(JSON.stringify(reply) + "\n");
  });
} else {
  const tokenIdx = process.argv.indexOf("--token");
  const token = tokenIdx > -1 ? process.argv[tokenIdx + 1] : null;
  const server = http.createServer((req, res) => {
    if (token && req.headers.authorization !== `Bearer ${token}`) {
      res.writeHead(401).end();
      return;
    }
    let body = "";
    req.on("data", (c) => (body += c));
    req.on("end", () => {
      const msg = JSON.parse(body);
      const reply = handle(msg);
      const headers = msg.method === "initialize" ? { "Mcp-Session-Id": "session-123" } : {};
      if (msg.method !== "initialize" && req.headers["mcp-session-id"] !== "session-123") {
        res.writeHead(400).end("missing session");
        return;
      }
      if (!reply) {
        res.writeHead(202, headers).end();
      } else if (msg.method === "tools/call") {
        // Stream the reply as SSE, after an unrelated notification.
        res.writeHead(200, { ...headers, "Content-Type": "text/event-stream" });
        res.write(
          `data: ${JSON.stringify({ jsonrpc: "2.0", method: "notifications/progress", params: {} })}\n\n`,
        );
        res.end(`event: message\ndata: ${JSON.stringify(reply)}\n\n`);
      } else {
        res.writeHead(200, { ...headers, "Content-Type": "application/json" }).end(JSON.stringify(reply));
      }
    });
  });
  server.listen(Number(process.argv[httpIdx + 1]) || 0, "127.0.0.1", () => {
    process.stdout.write(`PORT ${server.address().port}\n`);
  });
}
