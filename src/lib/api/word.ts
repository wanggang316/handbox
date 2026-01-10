/**
 * 单词本相关 API
 */

import { apiCall } from './index';
import type {
  CreateWordRequest,
  ListWordsRequest,
  ReviewWordRequest,
  UpdateWordRequest,
  Word,
  WordDetail,
  WordReview,
  TranslateWordRequest,
  TranslateWordResponse,
} from '../types';

export async function createWord(request: CreateWordRequest): Promise<Word> {
  return apiCall<Word>('word_create', { request });
}

export async function listWords(request?: ListWordsRequest): Promise<Word[]> {
  return apiCall<Word[]>('word_list', { request });
}

export async function getWord(wordId: string): Promise<WordDetail> {
  return apiCall<WordDetail>('word_get', { wordId });
}

export async function updateWord(request: UpdateWordRequest): Promise<Word> {
  return apiCall<Word>('word_update', { request });
}

export async function deleteWord(wordId: string): Promise<void> {
  return apiCall<void>('word_delete', { wordId });
}

export async function reviewWord(
  request: ReviewWordRequest
): Promise<WordReview> {
  return apiCall<WordReview>('word_review', { request });
}

export async function translateWord(
  request: TranslateWordRequest
): Promise<TranslateWordResponse> {
  return apiCall<TranslateWordResponse>('word_translate', { request });
}
