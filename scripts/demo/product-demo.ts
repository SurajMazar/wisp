/**
 * The end-to-end product demo: Kokoro narration over frames composed from Wisp's real renders.
 *
 *   pnpm tsx scripts/demo/product-demo.ts [--reuse]   → build/demo/wisp-product-demo.mp4
 *
 * 1. Narration: scripts/demo/narration.tsv spoken by Kokoro (crates/audio/examples/narrate.rs).
 * 2. Native renders: the island and guidance overlays, drawn by the app's own SwiftUI/AppKit code
 *    (IslandTests, GuidanceRenderTests write PNGs when asked).
 * 3. UI: real screens from the web UI against a real engine and your local model (capture-ui.ts).
 * 4. compose.swift lays them out on a desktop with captions; ffmpeg times each scene to its line.
 *
 * --reuse skips steps 1–3 when their output already exists.
 */
import { execFileSync } from "node:child_process";
import { existsSync, mkdirSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { homedir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "../..");
const out = join(root, "build/demo");
const assets = join(out, "assets");
const audio = join(assets, "audio");
const native = join(assets, "native");
const ui = join(assets, "ui");
const frames = join(assets, "frames");
const segments = join(assets, "segments");
const reuse = process.argv.includes("--reuse");
const run = (cmd: string, args: string[], env: Record<string, string> = {}) =>
  execFileSync(cmd, args, { cwd: root, stdio: "inherit", env: { ...process.env, ...env } });
const ready = (dir: string) => reuse && existsSync(dir) && readdirSync(dir).length > 0;

// ─── scenes ────────────────────────────────────────────────────────────────

type Shot = {
  /** Share of the scene's time; the last shot takes what's left. */
  share?: number;
  kind?: "desktop" | "title";
  window?: string;
  windowSize?: [number, number];
  island?: string;
  banner?: string;
  title?: string;
  subtitle?: string;
  caption?: string;
  ink?: [number, number, number, number];
  pointer?: [number, number];
};
const n = (f: string) => join(native, `${f}.png`);
const u = (f: string) => join(ui, `${f}.png`);
const MOCK: [number, number] = [1260, 728];
const PLAYER: [number, number] = [1280, 720];

const scenes: Record<string, Shot[]> = {
  "01-intro": [{ kind: "title", title: "Wisp", subtitle: "An AI companion that lives beside your cursor" }],
  "02-ask": [
    {
      share: 0.32,
      window: "@mockapp",
      windowSize: MOCK,
      island: n("island-listening"),
      pointer: [0.5, 0.45],
      caption: "Hold ⌃⌥ and ask: “How do I keep my changes?”",
    },
    {
      share: 0.14,
      window: "@mockapp",
      windowSize: MOCK,
      island: n("island-thinking"),
      pointer: [0.5, 0.45],
      caption: "Wisp looks at the window you're in",
    },
    {
      window: n("guidance-spotlight"),
      windowSize: MOCK,
      island: n("island-answer"),
      pointer: [0.79, 0.9],
      caption: "It answers out loud and points at what you need",
    },
  ],
  "03-draw": [
    {
      share: 0.55,
      window: join(assets, "player.jpg"),
      windowSize: PLAYER,
      island: n("island-drawing"),
      ink: [0.082, 0.955, 0.045, 0.05],
      caption: "While you hold the keys, your pointer becomes a pen",
    },
    {
      window: join(assets, "player.jpg"),
      windowSize: PLAYER,
      island: n("island-circled"),
      ink: [0.082, 0.955, 0.045, 0.05],
      caption: "Circle anything, and Wisp knows what you mean",
    },
  ],
  "04-chat": [
    { share: 0.24, window: u("chat-typed"), caption: "Prefer typing? It's the same assistant" },
    { share: 0.14, window: u("chat-thinking"), caption: "Answered by a model on your Mac" },
    { share: 0.26, window: u("chat-streaming"), caption: "Answered by a model on your Mac" },
    { window: u("chat-done"), caption: "Your whole conversation, kept on your Mac" },
  ],
  "05-agent": [
    {
      share: 0.45,
      window: u("home"),
      island: n("island-executing"),
      caption: "“Agent, remind me tomorrow at 9 to call Sam.”",
    },
    {
      window: u("home"),
      island: n("island-done"),
      banner: "Reminder set: Call Sam, tomorrow at 9:00 AM",
      caption: "Done in the background. Anything that changes something asks first",
    },
  ],
  "06-connect": [
    {
      window: u("connections"),
      caption: "Connect services through MCP, and choose Ask, Allow or Block per tool",
    },
  ],
  "07-dictate": [
    {
      window: u("dictation"),
      island: n("island-dictating"),
      caption: "Hold ⌃⇧ to dictate: your words appear where your cursor is",
    },
  ],
  "08-talk": [
    {
      share: 0.55,
      window: u("chat-done"),
      island: n("island-talking"),
      caption: "Tap twice for a hands-free conversation",
    },
    { window: u("chat-done"), island: n("island-speaking"), caption: "It never listens while it's speaking" },
  ],
  "09-models": [
    { share: 0.36, window: u("models"), caption: "Open models, running on your Mac" },
    { share: 0.32, window: u("models-cloud"), caption: "Or your own OpenAI, Anthropic or Gemini key" },
    { window: u("voice"), caption: "Choose the voice you like" },
  ],
  "10-outro": [
    {
      share: 0.5,
      window: u("privacy"),
      caption: "Local Only is the default: nothing leaves your Mac unless you say so",
    },
    { kind: "title", title: "Wisp", subtitle: "A companion for your Mac" },
  ],
};

// ─── steps ─────────────────────────────────────────────────────────────────

function narrate() {
  if (ready(audio)) return;
  const model = join(homedir(), "Library/Application Support/Wisp/models/kokoro-82m-full");
  run("cargo", [
    "run",
    "-q",
    "--release",
    "-p",
    "wisp-audio",
    "--example",
    "narrate",
    "--",
    model,
    join(root, "scripts/demo/narration.tsv"),
    audio,
  ]);
}

function renderNative() {
  if (ready(native)) return;
  mkdirSync(native, { recursive: true });
  run("scripts/development/swiftpm.sh", ["test", "--filter", "IslandTests|GuidanceRenderTests"], {
    WISP_ISLAND_SNAPSHOTS: native,
    WISP_GUIDANCE_SNAPSHOTS: native,
  });
}

function captureUi() {
  if (!ready(ui)) run("pnpm", ["tsx", "scripts/demo/capture-ui.ts", ui]);
}

/** A stand-in video player, drawn so the demo needs no real video. */
function player() {
  const path = join(assets, "player.jpg");
  if (existsSync(path)) return;
  const swift = `import AppKit
let W = 2560.0, H = 1440.0
let rep = NSBitmapImageRep(bitmapDataPlanes: nil, pixelsWide: Int(W), pixelsHigh: Int(H), bitsPerSample: 8, samplesPerPixel: 4, hasAlpha: true, isPlanar: false, colorSpaceName: .deviceRGB, bytesPerRow: 0, bitsPerPixel: 0)!
NSGraphicsContext.current = NSGraphicsContext(bitmapImageRep: rep)
NSColor(white: 0.08, alpha: 1).setFill(); CGRect(x: 0, y: 0, width: W, height: H).fill()
NSGradient(colors: [NSColor(srgbRed: 0.18, green: 0.30, blue: 0.52, alpha: 1), NSColor(srgbRed: 0.42, green: 0.27, blue: 0.55, alpha: 1)])!.draw(in: CGRect(x: 0, y: 160, width: W, height: H - 320), angle: 20)
("Mountain light · Episode 3" as NSString).draw(at: CGPoint(x: 80, y: H - 110), withAttributes: [.font: NSFont.systemFont(ofSize: 48, weight: .semibold), .foregroundColor: NSColor.white])
NSColor(white: 0.03, alpha: 1).setFill(); CGRect(x: 0, y: 0, width: W, height: 160).fill()
NSColor.gray.setFill(); CGRect(x: 0, y: 116, width: W, height: 8).fill()
NSColor.red.setFill(); CGRect(x: 0, y: 116, width: W * 0.39, height: 8).fill()
NSColor.white.setFill()
CGRect(x: 80, y: 40, width: 14, height: 48).fill(); CGRect(x: 108, y: 40, width: 14, height: 48).fill()
let p = NSBezierPath(); p.move(to: CGPoint(x: 180, y: 40)); p.line(to: CGPoint(x: 180, y: 88)); p.line(to: CGPoint(x: 216, y: 64)); p.close(); p.fill()
CGRect(x: 218, y: 40, width: 8, height: 48).fill()
try! rep.representation(using: .jpeg, properties: [.compressionFactor: 0.92])!.write(to: URL(fileURLWithPath: CommandLine.arguments[1]))
`;
  const src = join(assets, "player.swift");
  writeFileSync(src, swift);
  run("swift", [src, path]);
}

const seconds = (file: string) =>
  Number(
    execFileSync("ffprobe", ["-v", "error", "-show_entries", "format=duration", "-of", "csv=p=0", file], {
      encoding: "utf8",
    }).trim(),
  );

const LEAD = 0.25,
  TAIL = 0.55,
  XF = 0.35,
  FADE = 0.3;

function main() {
  mkdirSync(assets, { recursive: true });
  narrate();
  renderNative();
  captureUi();
  player();

  // Frames.
  rmSync(frames, { recursive: true, force: true });
  mkdirSync(frames, { recursive: true });
  const compose = join(assets, "compose");
  run("swiftc", ["-O", "scripts/demo/compose.swift", "-o", compose]);
  const mock = join(frames, "mockapp.png");
  const specs: Record<string, unknown>[] = [{ out: mock, kind: "mockapp" }];
  const plan: { id: string; length: number; shots: { file: string; length: number }[] }[] = [];
  for (const [id, shots] of Object.entries(scenes)) {
    const length = LEAD + seconds(join(audio, `${id}.wav`)) + TAIL;
    let used = 0;
    const timed = shots.map((s, i) => {
      const file = join(frames, `${id}-${i}.png`);
      const d = i === shots.length - 1 ? length - used : length * (s.share ?? 1);
      used += d;
      const { share: _share, ...frame } = s;
      specs.push({
        out: file,
        kind: s.kind ?? "desktop",
        ...frame,
        window: s.window === "@mockapp" ? mock : s.window,
      });
      return { file, length: d };
    });
    plan.push({ id, length, shots: timed });
  }
  const specFile = join(assets, "frames.json");
  writeFileSync(specFile, JSON.stringify(specs));
  run(compose, [specFile]);

  // One segment per scene: shots cross-fade, the line plays after a short lead, fades at the ends.
  rmSync(segments, { recursive: true, force: true });
  mkdirSync(segments, { recursive: true });
  const list: string[] = [];
  for (const scene of plan) {
    const args = ["-y", "-v", "error"];
    scene.shots.forEach((s, i) => {
      const last = i === scene.shots.length - 1;
      args.push(
        "-loop",
        "1",
        "-framerate",
        "30",
        "-t",
        (s.length + (last ? 0 : XF)).toFixed(3),
        "-i",
        s.file,
      );
    });
    args.push("-i", join(audio, `${scene.id}.wav`));
    const filters: string[] = scene.shots.map(
      (_, i) => `[${i}:v]scale=1920:1080,setsar=1,format=yuv420p[v${i}]`,
    );
    let label = "v0";
    let offset = 0;
    for (let i = 1; i < scene.shots.length; i++) {
      offset += scene.shots[i - 1]!.length;
      filters.push(
        `[${label}][v${i}]xfade=transition=fade:duration=${XF}:offset=${offset.toFixed(3)}[x${i}]`,
      );
      label = `x${i}`;
    }
    const end = (scene.length - FADE).toFixed(3);
    filters.push(`[${label}]fade=t=in:st=0:d=${FADE},fade=t=out:st=${end}:d=${FADE}[v]`);
    const a = scene.shots.length;
    filters.push(
      `[${a}:a]aresample=48000,adelay=${LEAD * 1000}:all=1,apad,atrim=0:${scene.length.toFixed(3)}[a]`,
    );
    const file = join(segments, `${scene.id}.mp4`);
    args.push(
      "-filter_complex",
      filters.join(";"),
      "-map",
      "[v]",
      "-map",
      "[a]",
      "-t",
      scene.length.toFixed(3),
      "-r",
      "30",
      "-c:v",
      "libx264",
      "-preset",
      "slow",
      "-crf",
      "18",
      "-pix_fmt",
      "yuv420p",
      "-c:a",
      "aac",
      "-b:a",
      "160k",
      "-ar",
      "48000",
      file,
    );
    run("ffmpeg", args);
    list.push(`file '${file}'`);
    console.log(`  ${scene.id}  ${scene.length.toFixed(1)} s`);
  }
  const listFile = join(segments, "list.txt");
  writeFileSync(listFile, list.join("\n") + "\n");
  const mp4 = join(out, "wisp-product-demo.mp4");
  run("ffmpeg", [
    "-y",
    "-v",
    "error",
    "-f",
    "concat",
    "-safe",
    "0",
    "-i",
    listFile,
    "-c",
    "copy",
    "-movflags",
    "+faststart",
    mp4,
  ]);
  console.log(`\n✓ ${mp4}  (${seconds(mp4).toFixed(1)} s)`);
}

main();
