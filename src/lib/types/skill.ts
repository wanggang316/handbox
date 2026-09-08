/** Mirrors the camelCase shape returned by the backend `skill_list` command. */

export type SkillScope = 'project' | 'user' | 'appData';

export interface SkillInfo {
  name: string;
  description: string | null;
  scope: SkillScope;
  /** Absolute path of the skill's directory. */
  path: string;
  /** SKILL.md body; null when validation failed. */
  body: string | null;
  /** Validation diagnostics; empty when valid. */
  diagnostics: string[];
  disabled: boolean;
}

/** One file inside a skill's directory, as returned by `skill_files`. */
export interface SkillFile {
  /** Path relative to the skill directory, always `/`-separated. */
  relPath: string;
  size: number;
  /** False for binary assets and oversized files; reading them is refused. */
  readable: boolean;
}
