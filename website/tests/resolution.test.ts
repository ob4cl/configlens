import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { resolveModel, type ModelSource } from "../src/lib/resolution.ts";
interface Fixture {
  id: string;
  client: string;
  values: string[];
  included: boolean[];
}
interface Check {
  fixture: string;
  observed: string;
  status: string;
  support: string;
  effectiveWithheld: boolean;
}
const fixtures: Fixture[] = JSON.parse(
  readFileSync(new URL("../src/data/fixtures.json", import.meta.url), "utf8"),
);
const captures = JSON.parse(
  readFileSync(new URL("../src/data/captures.json", import.meta.url), "utf8"),
);
const checks: Check[] = captures.checks;
test("every fixture has a genuine CLI parity record and a partial observed winner", () => {
  assert.equal(fixtures.length, 4);
  assert.equal(checks.length, fixtures.length);
  assert.deepEqual([...new Set(fixtures.map((f) => f.client))].sort(), [
    "claude",
    "opencode",
  ]);
  for (const fixture of fixtures) {
    assert.equal(fixture.values.length, 3);
    assert.equal(fixture.included.length, 3);
    const sources = fixture.values.map((value, i) => ({
      id: String(i),
      value,
      included: fixture.included[i] ?? false,
    }));
    const result = resolveModel(sources),
      check = checks.find((c) => c.fixture === fixture.id);
    assert.ok(check);
    assert.equal(result.winner?.value, check.observed);
    assert.equal(result.status, check.status);
    assert.equal(result.support, check.support);
    assert.equal(result.effectiveValue, null);
    assert.equal(check.effectiveWithheld, true);
  }
});
test("all inclusion combinations are deterministic, ordered and non-mutating", () => {
  for (let mask = 0; mask < 8; mask++) {
    const sources: ModelSource[] = [0, 1, 2].map((i) => ({
      id: String(i),
      value: `model-${i}`,
      included: Boolean(mask & (1 << i)),
    }));
    const copy = structuredClone(sources),
      result = resolveModel(sources),
      included = sources.filter((s) => s.included);
    assert.deepEqual(result.winner, included.at(-1) ?? null);
    assert.deepEqual(result.losers, included.slice(0, -1));
    assert.deepEqual(sources, copy);
    assert.deepEqual(resolveModel(sources), result);
    assert.equal(result.effectiveValue, null);
  }
});
test("empty input does not invent a runtime default", () => {
  assert.equal(resolveModel([]).winner, null);
  assert.equal(resolveModel([]).status, "partial");
});
test("empty, unicode and markup strings remain inert scalar data", () => {
  for (const value of ["", "<script>alert(1)</script>", "模型-α", '"quoted"'])
    assert.equal(
      resolveModel([{ id: "local", value, included: true }]).winner?.value,
      value,
    );
});
test("an excluded high-priority definition cannot replace an included one", () => {
  assert.equal(
    resolveModel([
      { id: "low", value: "low", included: true },
      { id: "high", value: "high", included: false },
    ]).winner?.id,
    "low",
  );
});
test("duplicates preserve provenance rather than collapsing source identity", () => {
  const result = resolveModel([
    { id: "user", value: "same", included: true },
    { id: "local", value: "same", included: true },
  ]);
  assert.equal(result.winner?.id, "local");
  assert.equal(result.losers[0]?.id, "user");
});
test("six captures retain commands, success exits and authentic coverage caveats", () => {
  for (const name of [
    "clients",
    "scan",
    "explain",
    "doctor",
    "snapshot",
    "diff",
  ]) {
    const entry = captures.commands[name];
    assert.ok(entry.command.startsWith(`configlens ${name}`));
    assert.equal(entry.exitCode, 0);
    assert.ok(entry.stdout || entry.stderr);
  }
  assert.match(captures.commands.explain.stdout, /effective value withheld/);
  assert.match(captures.commands.doctor.stdout, /never certifies/);
  assert.match(captures.commands.diff.stdout, /unknown_redacted/);
  assert.equal(captures.commands.redaction.projection.redacted, true);
  assert.ok(!JSON.stringify(captures).includes("synthetic-not-a-credential"));
});
