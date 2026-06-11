import { apiCall } from './index';
import type { SkillInfo } from '../types';

/**
 * 列出所有可用 skill（项目 / 用户 / 应用三档 scope，跨 scope 同名已去重）。
 *
 * @param workingDir 可选工作目录，用于解析项目级 skill。
 */
export async function listSkills(workingDir?: string): Promise<SkillInfo[]> {
  return apiCall<SkillInfo[]>('skill_list', { workingDir });
}
