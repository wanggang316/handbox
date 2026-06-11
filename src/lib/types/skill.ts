/**
 * Skill 类型定义
 *
 * 与后端 skill_list 命令返回的 camelCase 结构对齐。
 */

export type SkillScope = 'project' | 'user' | 'appData';

export interface SkillInfo {
  name: string;
  description: string | null;
  scope: SkillScope;
  /** skill 所在目录的绝对路径 */
  path: string;
  /** SKILL.md 正文；校验失败项为 null */
  body: string | null;
  /** 校验诊断信息；成功项为空数组 */
  diagnostics: string[];
}
