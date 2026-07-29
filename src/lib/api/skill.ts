import { apiCall } from './index';
import type { SkillInfo } from '../types';

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
