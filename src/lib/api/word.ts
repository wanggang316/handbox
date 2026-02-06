/**
 * 单词本相关 API
 */

import { apiCall } from "./index";
import type {
  CreateWordRequest,
  ListWordsRequest,
  UpdateWordRequest,
  Word,
  Message,
} from "../types";

export async function createWord(request: CreateWordRequest): Promise<Word> {
  return apiCall<Word>("word_create", { request });
}

export async function listWords(request?: ListWordsRequest): Promise<Word[]> {
  return apiCall<Word[]>("word_list", { request });
}

export async function getWord(wordId: string): Promise<Word> {
  return apiCall<Word>("word_get", { wordId });
}

export async function updateWord(request: UpdateWordRequest): Promise<Word> {
  return apiCall<Word>("word_update", { request });
}

export async function deleteWord(wordId: string): Promise<void> {
  return apiCall<void>("word_delete", { wordId });
}

export async function getTranslationHistory(
  sessionId: string,
  limit?: number,
  offset?: number,
): Promise<Message[]> {
  return apiCall<Message[]>("word_translation_history", {
    sessionId,
    limit,
    offset,
  });
}
