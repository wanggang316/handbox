export interface Word {
  id: string;
  term: string;
  language: string;
  translation: string;
  phonetic?: string | null;
  explanation?: string | null;
  note?: string | null;
  tags: string[];
  source: string;
  createdAt: number;
  updatedAt: number;
}

export interface CreateWordRequest {
  term: string;
  language: string;
  translation: string;
  phonetic?: string | null;
  explanation?: string | null;
  note?: string | null;
  tags?: string[];
  source: string;
}

export interface UpdateWordRequest {
  id: string;
  term?: string;
  language?: string;
  translation?: string;
  phonetic?: string | null;
  explanation?: string | null;
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
