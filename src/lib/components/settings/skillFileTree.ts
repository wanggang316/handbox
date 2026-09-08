import type { SkillFile } from "$lib/types";

/**
 * One line of the rendered tree: either a directory heading or a file.
 *
 * The tree is flattened here rather than rendered recursively so the picker
 * stays a flat list it can scroll, indent and key by path.
 */
export interface SkillTreeRow {
  /** Path relative to the skill directory; a directory's own path for a heading. */
  relPath: string;
  /** Final path segment — what the row shows. */
  name: string;
  /** Nesting level, 0 at the skill root. */
  depth: number;
  isDirectory: boolean;
  /** File rows only. */
  size?: number;
  readable?: boolean;
}

interface Node {
  name: string;
  path: string;
  children: Map<string, Node>;
  /** Set on leaves; its absence is what makes a node a directory. */
  file?: SkillFile;
}

function child(parent: Node, name: string): Node {
  const existing = parent.children.get(name);
  if (existing) return existing;
  const node: Node = {
    name,
    path: parent.path ? `${parent.path}/${name}` : name,
    children: new Map(),
  };
  parent.children.set(name, node);
  return node;
}

/**
 * Order within one level: files before the directories that subdivide them, so
 * reading top-down gives the content first and the drill-downs after. `SKILL.md`
 * is pinned to the very top of the root — it is the file the reader came for.
 */
function order(nodes: Node[], atRoot: boolean): Node[] {
  return [...nodes].sort((a, b) => {
    const aDir = a.file === undefined;
    const bDir = b.file === undefined;
    if (aDir !== bDir) return aDir ? 1 : -1;
    if (atRoot && !aDir) {
      if (a.name === "SKILL.md") return -1;
      if (b.name === "SKILL.md") return 1;
    }
    return a.name.localeCompare(b.name);
  });
}

/**
 * Turn a flat file listing into indented tree rows.
 *
 * Directories are implied by the paths — the listing carries only files — so a
 * directory row exists exactly when something under it does.
 */
export function buildSkillFileTree(files: SkillFile[]): SkillTreeRow[] {
  const root: Node = { name: "", path: "", children: new Map() };

  for (const file of files) {
    const segments = file.relPath.split("/").filter(Boolean);
    if (segments.length === 0) continue;
    let node = root;
    for (const segment of segments) {
      node = child(node, segment);
    }
    node.file = file;
  }

  const rows: SkillTreeRow[] = [];
  const walk = (node: Node, depth: number) => {
    for (const entry of order([...node.children.values()], depth === 0)) {
      if (entry.file) {
        rows.push({
          relPath: entry.file.relPath,
          name: entry.name,
          depth,
          isDirectory: false,
          size: entry.file.size,
          readable: entry.file.readable,
        });
      } else {
        rows.push({
          relPath: entry.path,
          name: entry.name,
          depth,
          isDirectory: true,
        });
        walk(entry, depth + 1);
      }
    }
  };
  walk(root, 0);

  return rows;
}
