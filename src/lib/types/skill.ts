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
