import type { SkillInfo } from '../types';
import * as skillApi from '../api/skill';

interface SkillStateData {
  skills: SkillInfo[];
  isLoading: boolean;
  error: string | null;
  initialized: boolean;
}

class SkillState {
  private state = $state<SkillStateData>({
    skills: [],
    isLoading: false,
    error: null,
    initialized: false,
  });

  get skills(): SkillInfo[] {
    return this.state.skills;
  }

  get isLoading(): boolean {
    return this.state.isLoading;
  }

  get error(): string | null {
    return this.state.error;
  }

  get initialized(): boolean {
    return this.state.initialized;
  }

  private setLoading(value: boolean) {
    this.state.isLoading = value;
  }

  private setError(message: string | null) {
    this.state.error = message;
  }

  private setSkills(skills: SkillInfo[]) {
    this.state.skills = skills;
  }

  async loadSkills(force = false): Promise<void> {
    if (this.state.isLoading) return;
    if (this.state.initialized && !force) return;

    try {
      this.setLoading(true);
      this.setError(null);
      const skills = await skillApi.listSkills();
      this.setSkills(skills);
      this.state.initialized = true;
    } catch (error) {
      const message = error instanceof Error ? error.message : '加载技能失败';
      this.setError(message);
      throw error;
    } finally {
      this.setLoading(false);
    }
  }
}

export const skillState = new SkillState();

export const skillActions = {
  loadSkills: (force = false) => skillState.loadSkills(force),
};
