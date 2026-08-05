import { apiCall } from './index';
import type { CreateHookRuleRequest, HookRule, UpdateHookRuleRequest } from '../types';

export async function listHookRules(): Promise<HookRule[]> {
  return apiCall<HookRule[]>('hook_rule_list');
}

export async function createHookRule(request: CreateHookRuleRequest): Promise<HookRule> {
  return apiCall<HookRule>('hook_rule_create', { request });
}

export async function updateHookRule(
  ruleId: string,
  request: UpdateHookRuleRequest
): Promise<HookRule> {
  return apiCall<HookRule>('hook_rule_update', { ruleId, request });
}

export async function deleteHookRule(ruleId: string): Promise<void> {
  await apiCall<void>('hook_rule_delete', { ruleId });
}
