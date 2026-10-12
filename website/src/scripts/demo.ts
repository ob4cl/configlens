import {
  fixtures,
  sourceInfo,
  type Client,
  type Fixture,
} from "../lib/fixtures";
import { resolveModel, type ModelSource } from "../lib/resolution";
import { timeline } from "../lib/motion";
import { element, text } from "../lib/dom";
export function setupDemo() {
  const initial = fixtures[0];
  if (!initial) return;
  let client: Client = "claude";
  let selected = 2;
  let current: Fixture = initial;
  let sources: ModelSource[] = [];
  const motion = timeline();
  const input = element<HTMLInputElement>("#model-value");
  const include = element<HTMLInputElement>("#include-source");
  const preset = element<HTMLSelectElement>("#preset");
  function inspect() {
    const source = sources[selected],
      info = sourceInfo[client][selected];
    if (!source || !info) return;
    text("#inspector-path", info.path);
    input.value = source.value;
    include.checked = source.included;
    const code = element(".json-code code");
    code.replaceChildren();
    if (!source.included) {
      code.textContent = "1  {}\n\n   /model is not defined in this source.";
      return;
    }
    const span = (cls: string, value: string) => {
      const node = document.createElement("span");
      node.className = cls;
      node.textContent = value;
      return node;
    };
    code.append(
      span("line-num", "1"),
      " {\n",
      span("line-num", "2"),
      "   ",
      span("json-key", '"model"'),
      ": ",
      span("json-string", JSON.stringify(source.value)),
      "\n",
      span("line-num", "3"),
      " }",
    );
  }
  function render() {
    const result = resolveModel(sources);
    sourceInfo[client].forEach((info, i) => {
      const source = sources[i];
      if (!source) return;
      text(`[data-label="${i}"]`, info.label);
      text(`[data-path="${i}"]`, info.path);
      text(
        `[data-value="${i}"]`,
        source.included ? source.value : "No /model definition",
      );
      const winner = result.winner?.id === source.id;
      text(
        `[data-badge="${i}"]`,
        !source.included
          ? "not defined"
          : winner
            ? "observed winner"
            : "overridden",
      );
      const row = element(`[data-row="${i}"]`);
      row.classList.toggle("is-winner", winner);
      row.classList.toggle("is-excluded", !source.included);
      row.classList.remove("is-tracing");
      element<HTMLButtonElement>(`[data-source="${i}"]`).setAttribute(
        "aria-pressed",
        String(selected === i),
      );
    });
    text(
      "#winner-value",
      result.winner ? result.winner.value : "No observed definition",
    );
    const winnerInfo = sourceInfo[client].find(
      (info) => info.id === result.winner?.id,
    );
    text(
      "#resolution-explanation",
      winnerInfo
        ? `${winnerInfo.label} wins among included sources; ${result.losers.length} earlier definition${result.losers.length === 1 ? " was" : "s were"} replaced.`
        : "No included source defines /model. This does not establish a runtime default.",
    );
    element("#resolve-demo").removeAttribute("aria-busy");
    inspect();
  }
  function cancel() {
    motion.cancel();
    render();
  }
  function load(fixture: Fixture) {
    motion.cancel();
    current = fixture;
    client = fixture.client;
    selected = 2;
    sources = sourceInfo[client].map((info, i) => ({
      id: info.id,
      value: fixture.values[i] ?? "",
      included: fixture.included[i] ?? false,
    }));
    document
      .querySelectorAll<HTMLButtonElement>("[data-client]")
      .forEach((button) =>
        button.setAttribute(
          "aria-pressed",
          String(button.dataset.client === client),
        ),
      );
    preset.replaceChildren(
      ...fixtures
        .filter((item) => item.client === client)
        .map((item) => {
          const option = document.createElement("option");
          option.value = item.id;
          option.textContent = item.label;
          return option;
        }),
    );
    preset.value = fixture.id;
    text(
      "#demo-scope",
      client === "claude"
        ? "Claude Code 2.1.296 · user → project shared → project local"
        : "OpenCode 1.18.35 · boundary: $PROJECT · cwd: packages/app",
    );
    text(
      "#coverage-detail",
      client === "claude"
        ? "Managed policy and server state are not observed here."
        : "Remote defaults, .opencode directory settings, and managed preferences remain coverage gaps.",
    );
    render();
  }
  document
    .querySelectorAll<HTMLButtonElement>("[data-client]")
    .forEach((button) =>
      button.addEventListener("click", () => {
        const next = fixtures.find((f) => f.client === button.dataset.client);
        if (next) load(next);
      }),
    );
  preset.addEventListener("change", () => {
    const next = fixtures.find((f) => f.id === preset.value);
    if (next) load(next);
  });
  document
    .querySelectorAll<HTMLButtonElement>("[data-source]")
    .forEach((button) =>
      button.addEventListener("click", () => {
        selected = Number(button.dataset.source);
        cancel();
      }),
    );
  input.addEventListener("input", () => {
    const source = sources[selected];
    if (!source) return;
    source.value = input.value;
    const position = input.selectionStart;
    cancel();
    input.setSelectionRange(position, position);
  });
  include.addEventListener("change", () => {
    const source = sources[selected];
    if (source) {
      source.included = include.checked;
      cancel();
    }
  });
  element("#reset-demo").addEventListener("click", () => load(current));
  element("#resolve-demo").addEventListener("click", () => {
    motion.cancel();
    render();
    element("#resolve-demo").setAttribute("aria-busy", "true");
    void motion.play(
      sources.length,
      (i) => {
        document
          .querySelectorAll("[data-row]")
          .forEach((row) => row.classList.remove("is-tracing"));
        element(`[data-row="${i}"]`).classList.add("is-tracing");
        document
          .querySelector("#trace-pulse")
          ?.setAttribute("cy", String(i * 89 + 45));
        const intermediate = resolveModel(sources.slice(0, i + 1));
        text(
          "#winner-value",
          intermediate.winner?.value ?? "No observed definition",
        );
        text(
          "#resolution-explanation",
          `Tracing ${sourceInfo[client][i]?.label}: ${sources[i]?.included ? "read /model" : "no definition; skip"}.`,
        );
      },
      render,
    );
  });
  load(initial);
}
