/** Deliberately bounded /model scalar replacement, not a client emulator. */
export interface ModelSource {
  id: string;
  value: string;
  included: boolean;
}
export function resolveModel(sources: readonly ModelSource[]) {
  const observed = sources.filter((source) => source.included);
  return {
    winner: observed.at(-1) ?? null,
    losers: observed.slice(0, -1),
    status: "partial" as const,
    effectiveValue: null,
    support: "fixture_verified" as const,
  };
}
