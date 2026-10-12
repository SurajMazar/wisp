// A tiny stdio MCP notes server shown in the product demo (no dependencies, notes kept in memory).
import process from "node:process";
import readline from "node:readline";

const notes = [];
const tools = [
  {
    name: "search_notes",
    title: "Search notes",
    description: "Find notes that mention some words.",
    inputSchema: { type: "object", properties: { query: { type: "string" } }, required: ["query"] },
    annotations: { readOnlyHint: true },
  },
  {
    name: "add_note",
    title: "Add note",
    description: "Save a new note.",
    inputSchema: {
      type: "object",
      properties: { title: { type: "string" }, body: { type: "string" } },
      required: ["title"],
    },
  },
  {
    name: "delete_note",
    title: "Delete note",
    description: "Remove a note for good.",
    inputSchema: { type: "object", properties: { title: { type: "string" } }, required: ["title"] },
    annotations: { destructiveHint: true },
  },
];

const text = (t) => ({ content: [{ type: "text", text: t }] });
function handle(msg) {
  const ok = (result) => ({ jsonrpc: "2.0", id: msg.id, result });
  const a = msg.params?.arguments ?? {};
  switch (msg.method) {
    case "initialize":
      return ok({
        protocolVersion: "2025-06-18",
        capabilities: { tools: {} },
        serverInfo: { name: "team-notes", version: "1.0.0" },
      });
    case "tools/list":
      return ok({ tools });
    case "tools/call":
      switch (msg.params?.name) {
        case "search_notes":
          return ok(
            text(
              notes
                .filter((n) => `${n.title} ${n.body}`.includes(a.query))
                .map((n) => n.title)
                .join("\n") || "No notes found.",
            ),
          );
        case "add_note":
          notes.push({ title: a.title, body: a.body ?? "" });
          return ok(text(`Saved “${a.title}”.`));
        case "delete_note":
          return ok(text(`Deleted “${a.title}”.`));
      }
      return { jsonrpc: "2.0", id: msg.id, error: { code: -32602, message: "Unknown tool" } };
    default:
      return { jsonrpc: "2.0", id: msg.id, error: { code: -32601, message: "Method not found" } };
  }
}

readline.createInterface({ input: process.stdin }).on("line", (line) => {
  if (!line.trim()) return;
  const msg = JSON.parse(line);
  if (msg.id === undefined || msg.method === undefined) return;
  process.stdout.write(JSON.stringify(handle(msg)) + "\n");
});
