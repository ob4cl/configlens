import { setupDemo } from "./demo";
import { element, text } from "../lib/dom";
import { timeline } from "../lib/motion";
import captures from "../data/captures.json";
setupDemo();
const menu = element<HTMLButtonElement>("#menu-toggle"),
  nav = element("#navigation");
function closeMenu(restore = false) {
  menu.setAttribute("aria-expanded", "false");
  nav.classList.remove("is-open");
  if (restore) menu.focus();
}
menu.addEventListener("click", () => {
  const open = menu.getAttribute("aria-expanded") !== "true";
  menu.setAttribute("aria-expanded", String(open));
  nav.classList.toggle("is-open", open);
});
document.addEventListener("keydown", (event) => {
  if (event.key === "Escape" && menu.getAttribute("aria-expanded") === "true")
    closeMenu(true);
});
nav
  .querySelectorAll("a")
  .forEach((link) => link.addEventListener("click", () => closeMenu()));
document.addEventListener("click", (event) => {
  if (
    event.target instanceof Node &&
    !nav.contains(event.target) &&
    !menu.contains(event.target)
  )
    closeMenu();
});
matchMedia("(min-width: 761px)").addEventListener("change", () => closeMenu());
const heroDetails = [
  "User contributes user-model. The shared and local project definitions replace it.",
  "Project shared contributes project-model. The local definition replaces it.",
  "Project local replaces earlier file definitions. Effective value withheld: coverage is partial.",
];
document
  .querySelectorAll<HTMLButtonElement>("[data-hero]")
  .forEach((button) =>
    button.addEventListener("click", () =>
      text("#hero-detail", heroDetails[Number(button.dataset.hero)] ?? ""),
    ),
  );
const stages = [
  "Known paths → candidate sources",
  "Definitions → ordered provenance",
  "Verified rule → observed winner",
  "Winner + gaps → bounded conclusion",
];
const pipeline = timeline();
function showStage(index: number) {
  document
    .querySelectorAll<HTMLElement>("[data-stage]")
    .forEach((node, i) => node.classList.toggle("active", i === index));
  document
    .querySelectorAll("[data-step]")
    .forEach((node, i) =>
      node.setAttribute("aria-pressed", String(i === index)),
    );
  text("#stage-output", stages[index] ?? "");
  document
    .querySelector("#pipeline-dot")
    ?.setAttribute("cx", String(8 + index * 68));
}
document.querySelectorAll<HTMLButtonElement>("[data-step]").forEach((button) =>
  button.addEventListener("click", () => {
    pipeline.cancel();
    showStage(Number(button.dataset.step));
  }),
);
element("#play-pipeline").addEventListener(
  "click",
  () => void pipeline.play(4, showStage, () => {}),
);
type CommandName =
  | "explain"
  | "clients"
  | "scan"
  | "doctor"
  | "snapshot"
  | "diff";
let command: CommandName = "explain";
let copyGeneration = 0;
document
  .querySelectorAll<HTMLButtonElement>("[data-command]")
  .forEach((button) =>
    button.addEventListener("click", () => {
      command = button.dataset.command as CommandName;
      copyGeneration++;
      const entry = captures.commands[command];
      text("#terminal-command", `$ ${entry.command}`);
      text(
        "#terminal-output",
        entry.stdout + (entry.stderr ? `\n[stderr]\n${entry.stderr}` : ""),
      );
      if (!matchMedia("(prefers-reduced-motion: reduce)").matches) {
        const output = element("#terminal-output");
        output.getAnimations().forEach((animation) => animation.cancel());
        output.animate(
          [
            { opacity: 0.35, transform: "translateY(4px)" },
            { opacity: 1, transform: "translateY(0)" },
          ],
          { duration: 180, easing: "ease-out" },
        );
      }
      text("#terminal-exit", `EXIT ${entry.exitCode} · FULL OUTPUT`);
      text("#copy-status", "");
      document
        .querySelectorAll<HTMLButtonElement>("[data-command]")
        .forEach((tab) =>
          tab.setAttribute("aria-pressed", String(tab === button)),
        );
    }),
  );
element("#copy-command").addEventListener("click", async () => {
  const generation = ++copyGeneration;
  try {
    await navigator.clipboard.writeText(captures.commands[command].command);
    if (generation === copyGeneration)
      text(
        "#copy-status",
        "Command copied. Review sandbox paths before running.",
      );
  } catch {
    if (generation === copyGeneration)
      text(
        "#copy-status",
        "Clipboard unavailable. Select the command text to copy.",
      );
  }
});
element<HTMLInputElement>("#show-projection").addEventListener(
  "change",
  (event) => {
    const checked = (event.target as HTMLInputElement).checked;
    text(
      "#redaction-output",
      checked
        ? JSON.stringify(captures.commands.redaction.projection, null, 2)
        : JSON.stringify(
            { env: { APP_TOKEN: "synthetic-not-a-credential" } },
            null,
            2,
          ),
    );
  },
);
let installCopyGeneration = 0;
document
  .querySelectorAll<HTMLButtonElement>("[data-copy-target]")
  .forEach((button) => {
    button.addEventListener("click", async () => {
      const target = document.getElementById(button.dataset.copyTarget ?? "");
      if (!target) return;
      const generation = ++installCopyGeneration;
      try {
        await navigator.clipboard.writeText(target.textContent ?? "");
        if (generation === installCopyGeneration)
          text(
            "#install-copy-status",
            "Command copied. Review it before running.",
          );
      } catch {
        if (generation === installCopyGeneration)
          text(
            "#install-copy-status",
            "Clipboard unavailable. Select the command text to copy.",
          );
      }
    });
  });
const platforms = {
  linux: {
    target: "x86_64-unknown-linux-gnu",
    extension: "tar.gz",
    tool: "sha256sum",
  },
  macos: {
    target: "aarch64-apple-darwin",
    extension: "tar.gz",
    tool: "shasum -a 256",
  },
  windows: {
    target: "x86_64-pc-windows-msvc",
    extension: "zip",
    tool: "Get-FileHash -Algorithm SHA256",
  },
};
element<HTMLSelectElement>("#platform").addEventListener("change", (event) => {
  const value = (event.target as HTMLSelectElement)
    .value as keyof typeof platforms;
  const platform = platforms[value];
  const file = `configlens-v0.1.0-beta.3-${platform.target}.${platform.extension}`;
  element<HTMLAnchorElement>("#download-link").href =
    `https://github.com/ob4cl/configlens/releases/download/v0.1.0-beta.3/${file}`;
  text("#checksum-command", `${platform.tool} ${file}`);
  const label =
    value === "macos" ? "macOS" : value === "windows" ? "Windows" : "Linux";
  text("#download-link", `Download ${label} archive ↓`);
  installCopyGeneration++;
  text("#install-copy-status", "");
});
element<HTMLSelectElement>("#platform").disabled = false;
