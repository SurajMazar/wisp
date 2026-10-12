/**
 * UI test of the settings and controls a person uses, in Chromium against a real `wisp-core`.
 * Each step clicks what a person would click, then checks what the engine actually saved.
 *
 *   pnpm ui:test [--shots <dir>]
 */
import { mkdirSync } from "node:fs";
import { join } from "node:path";
import { completeOnboarding, sleep, startApp } from "./harness";

const PORT = 4182;
const shotsArg = process.argv.indexOf("--shots");
const shots = shotsArg > -1 ? process.argv[shotsArg + 1] : undefined;

type Settings = {
  focus: { askContext: string; excludedApps: string[] };
  models: { deepThinking: boolean };
  privacy: { mode: string };
};

let passed = 0;
function check(cond: unknown, what: string): asserts cond {
  if (!cond) throw new Error(`✗ ${what}`);
  passed++;
  console.log(`  ✓ ${what}`);
}

async function main() {
  const app = await startApp({ port: PORT });
  const { page } = app;
  const shot = async (name: string) => {
    if (!shots) return;
    mkdirSync(shots, { recursive: true });
    await page.screenshot({ path: join(shots, `${name}.png`), fullPage: false });
  };
  const settings = () => app.call<Settings>("settings.get");
  const until = async (f: () => Promise<boolean>, what: string) => {
    for (let i = 0; i < 40; i++) {
      if (await f()) return;
      await sleep(100);
    }
    throw new Error(`✗ timed out: ${what}`);
  };

  try {
    console.log("First run");
    await completeOnboarding(page, PORT);
    check(true, "setup completes and the engine is running");

    console.log("Defaults (Step 1 privacy)");
    const s0 = await settings();
    check(
      s0.focus.askContext === "window",
      "spoken questions look at the window under the pointer by default",
    );
    check(
      s0.focus.excludedApps.includes("com.1password.1password"),
      "password managers are excluded by default",
    );
    check(s0.models.deepThinking === false, "deep thinking is off by default");

    console.log("Think toggle in the composer");
    const think = page.getByRole("button", { name: "Think" });
    await think.waitFor();
    check((await think.getAttribute("aria-pressed")) === "false", "Think starts off");
    const color = () => think.evaluate((el) => getComputedStyle(el).color);
    const offColor = await color();
    await think.click();
    await until(async () => (await settings()).models.deepThinking, "deep thinking saved");
    check((await think.getAttribute("aria-pressed")) === "true", "Think turns on and the engine saves it");
    await page.mouse.move(0, 0);
    await sleep(300); // let the colour transition finish
    check((await color()) !== offColor, "Think looks different when on");
    await shot("1-composer-think-on");
    await think.click();
    await until(async () => !(await settings()).models.deepThinking, "deep thinking off saved");
    check(true, "Think turns back off");

    console.log("Settings → Screen & focus");
    await page.goto(`http://127.0.0.1:${PORT}/#/settings`);
    const ask = page.getByRole("radiogroup", { name: "Screen context for spoken questions" });
    await ask.waitFor();
    check(
      (await ask.getByRole("radio", { name: "Window" }).getAttribute("aria-checked")) === "true",
      "“Window” is selected",
    );
    await ask.getByRole("radio", { name: "Whole screen" }).click();
    await until(async () => (await settings()).focus.askContext === "display", "askContext saved");
    check(true, "choosing “Whole screen” saves it");
    await ask.scrollIntoViewIfNeeded();
    await shot("2-settings-ask-context");
    await page.reload();
    await ask.waitFor();
    check(
      (await ask.getByRole("radio", { name: "Whole screen" }).getAttribute("aria-checked")) === "true",
      "the choice is still there after a reload",
    );
    await ask.getByRole("radio", { name: "Nothing" }).click();
    await until(async () => (await settings()).focus.askContext === "off", "off saved");
    check(true, "choosing “Nothing” saves it");
    await ask.getByRole("radio", { name: "Window" }).click();
    await until(async () => (await settings()).focus.askContext === "window", "window saved");
    check((await settings()).focus.excludedApps.length > 0, "changing it keeps the excluded apps");

    console.log("Settings → Models");
    await page.goto(`http://127.0.0.1:${PORT}/#/models`);
    const deep = page.getByRole("switch", { name: "Deep thinking" });
    await deep.waitFor();
    check((await deep.getAttribute("aria-checked")) === "false", "Deep thinking switch shows off");
    await deep.click();
    await until(async () => (await settings()).models.deepThinking, "switch saved");
    check(true, "the switch saves deep thinking");
    await deep.click();
    await until(async () => !(await settings()).models.deepThinking, "switch off saved");

    for (const name of ["OpenAI", "Anthropic", "Google Gemini"]) {
      const row = page.getByText(name, { exact: true }).first();
      check(await row.isVisible(), `Cloud models lists ${name}`);
    }
    check((await page.getByText("Not connected").count()) === 3, "all three show “Not connected”");
    check(
      !(await page
        .getByText("Your privacy setting is Local only")
        .isVisible()
        .catch(() => false)),
      "no privacy warning before any key is added",
    );
    await page.getByRole("button", { name: "Add Key" }).first().click();
    const key = page.getByLabel("OpenAI");
    check(await key.isVisible(), "Add Key opens a key field");
    check((await key.getAttribute("type")) === "password", "the key field hides what's typed");
    check(
      await page.getByRole("link", { name: "Get a key from OpenAI" }).isVisible(),
      "it links to where to get a key",
    );
    check(
      await page.getByRole("button", { name: "Save" }).isDisabled(),
      "Save is disabled until a key is typed",
    );
    await page.getByText("Cloud models").scrollIntoViewIfNeeded();
    await shot("3-models-cloud");
    await page.getByRole("button", { name: "Cancel" }).click();
    check(await page.getByRole("button", { name: "Add Key" }).first().isVisible(), "Cancel closes the field");
    check((await settings()).privacy.mode === "localOnly", "nothing about privacy changed");

    console.log(`\n✓ UI test passed (${passed} checks)`);
  } finally {
    await app.close();
  }
}

main().catch((e) => {
  console.error(e instanceof Error ? e.message : e);
  process.exit(1);
});
