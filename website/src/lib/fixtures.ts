import data from "../data/fixtures.json";
export type Client = "claude" | "opencode";
export interface Fixture {
  id: string;
  client: Client;
  label: string;
  version: string;
  values: string[];
  included: boolean[];
}
export const fixtures = data as Fixture[];
export const sourceInfo = {
  claude: [
    { label: "User", path: "$HOME/.claude/settings.json", id: "cc.user" },
    {
      label: "Project shared",
      path: "$PROJECT/.claude/settings.json",
      id: "cc.project_shared",
    },
    {
      label: "Project local",
      path: "$PROJECT/.claude/settings.local.json",
      id: "cc.project_local",
    },
  ],
  opencode: [
    {
      label: "Global",
      path: "$XDG_CONFIG/opencode/opencode.json",
      id: "oc.global",
    },
    {
      label: "Project ancestor",
      path: "$PROJECT/opencode.json",
      id: "oc.project.outer",
    },
    {
      label: "Project nested",
      path: "$PROJECT/packages/app/opencode.json",
      id: "oc.project.inner",
    },
  ],
};
