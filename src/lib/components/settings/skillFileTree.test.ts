import { describe, it, expect } from "vitest";
import { buildSkillFileTree } from "./skillFileTree";
import type { SkillFile } from "$lib/types";

function file(relPath: string, over: Partial<SkillFile> = {}): SkillFile {
  return { relPath, size: 10, readable: true, ...over };
}

describe("buildSkillFileTree", () => {
  it("returns nothing for an empty listing", () => {
    expect(buildSkillFileTree([])).toEqual([]);
  });

  it("pins SKILL.md above the other root files", () => {
    const rows = buildSkillFileTree([
      file("README.md"),
      file("AGENTS.md"),
      file("SKILL.md"),
    ]);
    expect(rows.map((r) => r.name)).toEqual([
      "SKILL.md",
      "AGENTS.md",
      "README.md",
    ]);
    expect(rows.every((r) => r.depth === 0)).toBe(true);
  });

  it("puts files before the directories at every level", () => {
    const rows = buildSkillFileTree([
      file("scripts/lib/util.mjs"),
      file("scripts/run.mjs"),
      file("SKILL.md"),
    ]);
    expect(rows.map((r) => [r.name, r.depth, r.isDirectory])).toEqual([
      ["SKILL.md", 0, false],
      ["scripts", 0, true],
      ["run.mjs", 1, false],
      ["lib", 1, true],
      ["util.mjs", 2, false],
    ]);
  });

  // Directories are implied by the paths: the listing carries only files, so a
  // heading exists exactly when something sits under it.
  it("names a directory row by its own path, not its first child", () => {
    const rows = buildSkillFileTree([file("references/deep/notes.md")]);
    expect(rows.map((r) => r.relPath)).toEqual([
      "references",
      "references/deep",
      "references/deep/notes.md",
    ]);
  });

  it("carries size and readability onto file rows only", () => {
    const rows = buildSkillFileTree([
      file("assets/logo.png", { size: 4096, readable: false }),
    ]);
    const [dir, leaf] = rows;
    expect(dir.isDirectory).toBe(true);
    expect(dir.size).toBeUndefined();
    expect(leaf).toMatchObject({ size: 4096, readable: false, depth: 1 });
  });
});
