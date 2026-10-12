import { chromium } from "@playwright/test";
import AxeBuilder from "@axe-core/playwright";
import { readFile, mkdir, writeFile, stat } from "node:fs/promises";
import { resolve, extname, sep } from "node:path";
import { tmpdir } from "node:os";
import assert from "node:assert/strict";

// No HTTP listener, dev server, preview server, API server or test webServer.
// Every browser request is fulfilled from the production artifact or blocked.
const liveSite = process.env.LIVE_SITE_URL;
const origin = liveSite ? new URL(liveSite).origin : "https://configlens.test";
const base = "/configlens/";
const dist = resolve("dist");
const screenshots =
  process.env.SCREENSHOT_DIR ||
  resolve(process.env.TMPDIR || tmpdir(), "configlens-website-screenshots");
await mkdir(screenshots, { recursive: true });
const browser = await chromium.launch({
  headless: true,
  executablePath: process.env.CHROMIUM_PATH,
  args: [
    "--no-sandbox",
    "--disable-background-networking",
    "--disable-component-update",
  ],
});
const findings = {
  viewports: [],
  interactions: [],
  requests: [],
  consoleErrors: [],
  screenshots: [],
};
const mime = {
  ".html": "text/html",
  ".css": "text/css",
  ".js": "text/javascript",
  ".svg": "image/svg+xml",
  ".png": "image/png",
  ".xml": "application/xml",
  ".txt": "text/plain",
};
async function route(context) {
  if (liveSite) return;
  await context.route("**/*", async (route) => {
    const url = new URL(route.request().url());
    if (url.origin !== origin || !url.pathname.startsWith(base)) {
      findings.requests.push(`BLOCKED ${url}`);
      return route.abort();
    }
    const local = resolve(
      dist,
      decodeURIComponent(url.pathname.slice(base.length)) || "index.html",
    );
    if (!local.startsWith(dist + sep)) return route.abort();
    try {
      await stat(local);
      await route.fulfill({
        status: 200,
        body: await readFile(local),
        contentType: mime[extname(local)] || "application/octet-stream",
      });
    } catch {
      findings.requests.push(`MISSING ${url}`);
      await route.fulfill({ status: 404, body: "Not found" });
    }
  });
}
async function check(page, label) {
  const audit = await new AxeBuilder({ page })
    .withTags(["wcag2a", "wcag2aa", "wcag21a", "wcag21aa", "wcag22aa"])
    .analyze();
  assert.deepEqual(
    audit.violations.map((v) => ({
      id: v.id,
      nodes: v.nodes.map((n) => n.target),
    })),
    [],
    `${label}: accessibility`,
  );
  // This experimental axe rule is not enabled by WCAG tag filtering alone.
  const labelAudit = await new AxeBuilder({ page })
    .withRules(["label-content-name-mismatch"])
    .analyze();
  assert.deepEqual(
    labelAudit.violations.map((v) => ({
      id: v.id,
      nodes: v.nodes.map((n) => n.target),
    })),
    [],
    `${label}: visible label must be included in accessible name`,
  );
}
try {
  for (const width of [1440, 1024, 768, 390, 320]) {
    const context = await browser.newContext({
      viewport: { width, height: 1000 },
      reducedMotion: "reduce",
      hasTouch: width <= 390,
      isMobile: width <= 390,
    });
    await route(context);
    const page = await context.newPage();
    page.on("pageerror", (e) => findings.consoleErrors.push(e.message));
    page.on("console", (msg) => {
      if (msg.type() === "error") findings.consoleErrors.push(msg.text());
    });
    await page.goto(origin + base);
    assert.ok(
      await page
        .getByRole("link", { name: "ConfigLens BETA home", exact: true })
        .isVisible(),
    );
    await page.waitForFunction(() =>
      document.querySelector('[data-row="2"]')?.classList.contains("is-winner"),
    );
    const overflow = await page.evaluate(() => ({
      width: innerWidth,
      scroll: document.documentElement.scrollWidth,
      offenders: [...document.querySelectorAll("body *")]
        .filter(
          (e) =>
            e.getBoundingClientRect().right > innerWidth + 1 &&
            getComputedStyle(e).position !== "fixed",
        )
        .map((e) => `${e.tagName}.${e.className}`),
    }));
    assert.ok(overflow.scroll <= width, JSON.stringify(overflow));
    await check(page, `${width}px initial`);
    const name = `configlens-${width}.png`;
    await page.screenshot({ path: resolve(screenshots, name), fullPage: true });
    findings.screenshots.push(resolve(screenshots, name));
    findings.viewports.push({ width, overflow: false, axeViolations: 0 });
    if (width === 1440) {
      await page.locator('[data-hero="0"]').click();
      assert.match(
        await page.locator("#hero-detail").textContent(),
        /User contributes/,
      );
      await page.locator('[data-hero="1"]').click();
      assert.match(
        await page.locator("#hero-detail").textContent(),
        /Project shared contributes/,
      );
      await page.locator('[data-hero="2"]').click();
      assert.match(
        await page.locator("#hero-detail").textContent(),
        /Effective value withheld/,
      );
      await page.locator(".hero-command summary").click();
      assert.ok(await page.locator(".hero-command code").isVisible());
      await page.locator(".hero-command summary").click();
      findings.interactions.push(
        "three hero sources and first-command disclosure",
      );
      await page.locator("#demo").screenshot({
        path: resolve(screenshots, "configlens-demo-desktop.png"),
      });
      assert.equal(
        await page.locator("#winner-value").textContent(),
        "local-model",
      );
      await page.locator("#include-source").uncheck();
      assert.equal(
        await page.locator("#winner-value").textContent(),
        "project-model",
      );
      await page.locator('[data-source="1"]').click();
      await page.locator("#include-source").uncheck();
      assert.equal(
        await page.locator("#winner-value").textContent(),
        "user-model",
      );
      await page.locator('[data-source="0"]').click();
      await page.locator("#include-source").uncheck();
      assert.equal(
        await page.locator("#winner-value").textContent(),
        "No observed definition",
      );
      await page.locator("#reset-demo").click();
      assert.equal(
        await page.locator("#winner-value").textContent(),
        "local-model",
      );
      await page.locator("#model-value").fill("<img src=x onerror=alert(1)>");
      assert.equal(
        await page.locator("#winner-value").textContent(),
        "<img src=x onerror=alert(1)>",
      );
      assert.equal(await page.locator(".json-code img").count(), 0);
      await page.locator("#model-value").fill("");
      assert.equal(await page.locator("#winner-value").textContent(), "");
      await page.locator("#reset-demo").click();
      await page.locator("#preset").selectOption("claude-shared");
      assert.equal(
        await page.locator("#winner-value").textContent(),
        "team-model",
      );
      await page.locator('[data-client="opencode"]').click();
      assert.equal(
        await page.locator("#winner-value").textContent(),
        "nested-model",
      );
      await page.locator("#preset").selectOption("opencode-root");
      assert.equal(
        await page.locator("#winner-value").textContent(),
        "workspace-model",
      );
      await page.locator("#resolve-demo").click();
      assert.equal(
        await page.locator("#winner-value").textContent(),
        "workspace-model",
      );
      assert.equal(
        await page.locator("#resolve-demo").getAttribute("aria-busy"),
        null,
      );
      await page.locator("#play-pipeline").click();
      assert.equal(
        await page.locator("#stage-output").textContent(),
        "Winner + gaps → bounded conclusion",
      );
      await page.locator('[data-step="1"]').click();
      assert.equal(
        await page.locator("#stage-output").textContent(),
        "Definitions → ordered provenance",
      );
      await page.locator("#show-projection").uncheck();
      assert.match(
        await page.locator("#redaction-output").textContent(),
        /synthetic-not-a-credential/,
      );
      await page.locator("#show-projection").check();
      assert.doesNotMatch(
        await page.locator("#redaction-output").textContent(),
        /synthetic-not-a-credential/,
      );
      for (const command of [
        "clients",
        "scan",
        "explain",
        "doctor",
        "snapshot",
        "diff",
      ]) {
        await page.locator(`[data-command="${command}"]`).click();
        assert.match(
          await page.locator("#terminal-command").textContent(),
          new RegExp(`configlens ${command}`),
        );
        assert.ok(
          (await page.locator("#terminal-output").textContent()).length > 0,
        );
      }
      // Cover deterministic clipboard success and failure without OS clipboard permission.
      await page.evaluate(() =>
        Object.defineProperty(navigator, "clipboard", {
          configurable: true,
          value: {
            writeText: async (value) => {
              window.__copiedCommand = value;
            },
          },
        }),
      );
      await page.locator("#copy-command").click();
      await page.waitForFunction(() =>
        document
          .querySelector("#copy-status")
          .textContent.includes("Command copied"),
      );
      assert.match(
        await page.evaluate(() => window.__copiedCommand),
        /^configlens diff/,
      );
      await page.evaluate(() =>
        Object.defineProperty(navigator, "clipboard", {
          configurable: true,
          value: {
            writeText: async () => {
              throw new Error("denied");
            },
          },
        }),
      );
      await page.locator("#copy-command").click();
      await page.waitForFunction(() =>
        document
          .querySelector("#copy-status")
          .textContent.includes("Clipboard unavailable"),
      );
      await page
        .locator(".verify-details")
        .evaluate((node) => (node.open = true));
      for (const [platform, target, hash] of [
        ["macos", "aarch64-apple-darwin", "shasum -a 256"],
        ["windows", "x86_64-pc-windows-msvc", "Get-FileHash"],
        ["linux", "x86_64-unknown-linux-gnu", "sha256sum"],
      ]) {
        await page.locator("#platform").selectOption(platform);
        assert.match(
          await page.locator("#download-link").getAttribute("href"),
          new RegExp(target),
        );
        assert.ok(
          (await page.locator("#checksum-command").textContent()).startsWith(
            hash,
          ),
        );
      }
      await page.locator('[data-copy-target="source-install-command"]').click();
      await page.waitForFunction(() =>
        document
          .querySelector("#install-copy-status")
          .textContent.includes("Clipboard unavailable"),
      );
      await page.evaluate(() =>
        Object.defineProperty(navigator, "clipboard", {
          configurable: true,
          value: {
            writeText: async (value) => {
              window.__copiedCommand = value;
            },
          },
        }),
      );
      for (const target of [
        "checksum-command",
        "source-install-command",
        "first-trace-command",
      ]) {
        await page.locator(`[data-copy-target="${target}"]`).click();
        await page.waitForFunction(() =>
          document
            .querySelector("#install-copy-status")
            .textContent.includes("Command copied"),
        );
        assert.equal(
          await page.evaluate(() => window.__copiedCommand),
          await page.locator(`#${target}`).textContent(),
        );
      }
      findings.interactions.push("installation clipboard success/failure");
      for (const summary of await page.locator(".feature summary").all())
        await summary.click();
      await check(page, "all disclosure / changed states");
      // Normal-motion timeline must not resurrect stale results after reset/client/input.
      await page.emulateMedia({ reducedMotion: "no-preference" });
      await page.locator('[data-client="claude"]').click();
      await page.locator("#resolve-demo").click();
      await page.locator("#reset-demo").click();
      await page.waitForTimeout(1100);
      assert.equal(
        await page.locator("#winner-value").textContent(),
        "local-model",
      );
      await page.locator("#resolve-demo").click();
      await page.locator('[data-client="opencode"]').click();
      await page.waitForTimeout(1100);
      assert.equal(
        await page.locator("#winner-value").textContent(),
        "nested-model",
      );
      await page.locator("#resolve-demo").click();
      await page.locator("#model-value").fill("changed-during-replay");
      await page.waitForTimeout(1100);
      assert.equal(
        await page.locator("#winner-value").textContent(),
        "changed-during-replay",
      );
      await page.locator("#resolve-demo").click();
      await page.waitForTimeout(1100);
      assert.equal(
        await page.locator("#resolve-demo").getAttribute("aria-busy"),
        null,
      );
      findings.interactions.push(
        "four presets",
        "all excluded",
        "scalar edit and HTML inertness",
        "empty scalar",
        "reset",
        "reduced motion immediate",
        "normal motion and stale cancellation",
        "four pipeline stages",
        "six captures",
        "clipboard success/failure",
        "protected projection",
        "three platform links/checksums",
        "six feature disclosures",
      );
    }
    if (width === 390 || width === 320) {
      await page.locator('[data-client="opencode"]').tap();
      assert.equal(
        await page.locator("#winner-value").textContent(),
        "nested-model",
      );
      await page.locator("#include-source").tap();
      assert.equal(
        await page.locator("#winner-value").textContent(),
        "workspace-model",
      );
      await page.locator("#reset-demo").tap();
      await page.locator('[data-client="claude"]').tap();
      findings.interactions.push(`${width}px touch client / include / reset`);
      await page.locator("#menu-toggle").focus();
      await page.keyboard.press("Enter");
      assert.equal(
        await page.locator("#menu-toggle").getAttribute("aria-expanded"),
        "true",
      );
      await page.keyboard.press("Tab");
      assert.equal(
        await page.evaluate(() => document.activeElement?.textContent),
        "Features",
      );
      await page.keyboard.press("Escape");
      assert.equal(
        await page.locator("#menu-toggle").getAttribute("aria-expanded"),
        "false",
      );
      assert.equal(
        await page.evaluate(() => document.activeElement?.id),
        "menu-toggle",
      );
      await page.locator('[data-client="opencode"]').focus();
      await page.keyboard.press("Enter");
      assert.equal(
        await page.locator("#winner-value").textContent(),
        "nested-model",
      );
      await page.locator("#include-source").focus();
      await page.keyboard.press("Space");
      assert.equal(
        await page.locator("#winner-value").textContent(),
        "workspace-model",
      );
      await page.locator("#reset-demo").click();
      await page.locator('[data-client="claude"]').click();
      await page.locator("#demo").screenshot({
        path: resolve(screenshots, `configlens-demo-${width}.png`),
      });
      findings.interactions.push(
        `${width}px keyboard menu / Escape / focus / client / checkbox`,
      );
    }
    await context.close();
  }
  const context = await browser.newContext({
    javaScriptEnabled: false,
    viewport: { width: 390, height: 900 },
  });
  await route(context);
  const page = await context.newPage();
  await page.goto(origin + base);
  assert.match(await page.locator("h1").textContent(), /Know exactly/);
  assert.equal(
    await page.locator("#winner-value").textContent(),
    "local-model",
  );
  assert.ok(await page.locator("noscript").first().isVisible());
  assert.ok(await page.locator("#platform").isDisabled());
  assert.match(await page.locator("#download-link").textContent(), /Linux/);
  const fallback = page.locator("#install noscript");
  assert.ok(await fallback.isVisible());
  assert.equal(await fallback.locator("a").count(), 3);
  for (const [label, target] of [
    ["Linux x86-64 GNU", "x86_64-unknown-linux-gnu.tar.gz"],
    ["macOS Apple Silicon", "aarch64-apple-darwin.tar.gz"],
    ["Windows x86-64", "x86_64-pc-windows-msvc.zip"],
  ]) {
    assert.ok(
      (
        await fallback
          .getByRole("link", { name: label, exact: true })
          .getAttribute("href")
      ).endsWith(target),
    );
  }
  findings.interactions.push(
    "no-JavaScript content, default trace and correct platform downloads",
  );
  await context.close();
  assert.deepEqual(findings.consoleErrors, []);
  assert.deepEqual(findings.requests, []);
  await writeFile(
    resolve(screenshots, "browser-results.json"),
    JSON.stringify(findings, null, 2),
  );
  console.log(JSON.stringify(findings, null, 2));
} catch (error) {
  await writeFile(
    resolve(screenshots, "browser-failure.json"),
    JSON.stringify({ message: String(error), findings }, null, 2),
  );
  throw error;
} finally {
  await browser.close();
}
