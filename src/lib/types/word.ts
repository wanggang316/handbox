export interface Word {
  id: string;
  term: string;
  language: string;
  translation: string;
  phonetic?: string | null;
  note?: string | null;
  tags: string[];
  source: string;
  createdAt: number;
  updatedAt: number;
}

export interface WordContext {
  id: string;
  wordId: string;
  contextText: string;
  sourceType: string;
  sourceId?: string | null;
  createdAt: number;
}

export interface WordReview {
  wordId: string;
  ease: number;
  intervalDays: number;
  nextReviewAt: number;
  lastReviewedAt?: number | null;
  reviewCount: number;
}

export interface WordDetail {
  word: Word;
  contexts: WordContext[];
  review?: WordReview | null;
}

export interface CreateWordContextRequest {
  contextText: string;
  sourceType: string;
  sourceId?: string | null;
}

export interface CreateWordRequest {
  term: string;
  language: string;
  translation: string;
  phonetic?: string | null;
  note?: string | null;
  tags?: string[];
  source: string;
  context?: CreateWordContextRequest | null;
}

export interface UpdateWordRequest {
  id: string;
  term?: string;
  language?: string;
  translation?: string;
  phonetic?: string | null;
  note?: string | null;
  tags?: string[];
  source?: string;
}

export interface ListWordsRequest {
  query?: string;
  tag?: string;
  limit?: number;
  offset?: number;
}

export interface ReviewWordRequest {
  wordId: string;
  remembered: boolean;
}

export interface TranslateWordRequest {
  term: string;
}

export interface TranslateWordResponse {
  term: string;
  translation: string;
  targetLanguage: string;
}
