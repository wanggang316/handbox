/**
 * 单词本相关 API
 */

import { apiCall } from './index';
import { sendUserMessageStream } from './message';
import { listenToStreamEvents, type StreamEventHandlers } from './message';
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
  CreateWordLookupRequest,
  ListWordLookupHistoryRequest,
  WordLookupHistory,
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

export async function recordLookup(
  request: CreateWordLookupRequest
): Promise<WordLookupHistory> {
  return apiCall<WordLookupHistory>('word_lookup_record', { request });
}

export async function listLookupHistory(
  request?: ListWordLookupHistoryRequest
): Promise<WordLookupHistory[]> {
  return apiCall<WordLookupHistory[]>('word_lookup_history', { request });
}

export async function deleteLookupHistory(historyId: string): Promise<void> {
  return apiCall<void>('word_lookup_delete', { historyId });
}

/**
 * 流式翻译单词
 * 使用 Session 的 sendUserMessageStream 接口
 */
export async function translateWordStream(
  sessionId: string,
  term: string,
  handlers: {
    onChunk?: (content: string) => void;
    onComplete?: (result: TranslateWordResponse) => void;
    onError?: (error: any) => void;
  }
): Promise<() => void> {
  // 发送流式消息 (注意：UserMessageSendRequest 使用 chatId 字段)
  await sendUserMessageStream({
    chatId: sessionId,
    content: term,
    tempUserMessageId: `trans-${Date.now()}`,
  });

  // 监听流式事件
  const unlisten = await listenToStreamEvents({
    onChunk: (data) => {
      handlers.onChunk?.(data.content);
    },
    onEnd: (data) => {
      // 解析翻译结果
      const result = parseTranslationResponse(data.finalContent, term);
      handlers.onComplete?.(result);
    },
    onError: (error) => {
      handlers.onError?.(error);
    },
  });

  return unlisten;
}

/**
 * 解析翻译响应
 * 从 LLM 的 JSON 响应中提取翻译结果
 */
export function parseTranslationResponse(
  content: string,
  term: string
): TranslateWordResponse {
  try {
    // 尝试解析 JSON 响应
    const jsonMatch = content.match(/\{[\s\S]*\}/);
    if (jsonMatch) {
      const parsed = JSON.parse(jsonMatch[0]);
      return {
        term,
        translation: parsed.translation || content,
        targetLanguage: parsed.targetLanguage || 'unknown',
        phonetic: parsed.phonetic || null,
        explanation: parsed.explanation || null,
      };
    }

    // 如果没有 JSON，直接返回内容作为翻译
    return {
      term,
      translation: content,
      targetLanguage: 'unknown',
      phonetic: null,
      explanation: null,
    };
  } catch (error) {
    console.error('Failed to parse translation response:', error);
    // 解析失败，返回原始内容
    return {
      term,
      translation: content,
      targetLanguage: 'unknown',
      phonetic: null,
      explanation: null,
    };
  }
}
