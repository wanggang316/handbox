import { apiCall } from './index';
import type { SkillFile, SkillInfo } from '../types';

/**
 * Lists skills across the project / user / app scopes, deduplicated by name.
 * `workingDir` is needed to resolve project-scope skills.
 */
export async function listSkills(workingDir?: string): Promise<SkillInfo[]> {
  return apiCall<SkillInfo[]>('skill_list', { workingDir });
}

export async function setSkillDisabled(name: string, disabled: boolean): Promise<void> {
  return apiCall<void>('skill_set_disabled', { name, disabled });
}

/**
 * Lists the files that make up a skill, for the detail view's file switcher.
 * Addressed by name rather than path: the backend re-resolves the directory
 * through discovery, so `workingDir` must match the one used to list.
 */
export async function listSkillFiles(name: string, workingDir?: string): Promise<SkillFile[]> {
  return apiCall<SkillFile[]>('skill_files', { name, workingDir });
}

/** Reads one file from a skill's directory as UTF-8 text. */
export async function readSkillFile(
  name: string,
  relPath: string,
  workingDir?: string
): Promise<string> {
  return apiCall<string>('skill_file_read', { name, relPath, workingDir });
}
