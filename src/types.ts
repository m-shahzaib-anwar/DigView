export type Category =
  "images" | "videos" | "music" | "documents" | "archives" | "apps" | "other";
export type View =
  "home" | "all" | Category | "sources" | "cleanup" | "duplicates" | "settings";
export interface Root {
  id: number;
  path: string;
  added_at: number;
  last_scan: number | null;
}
export interface HiddenItem {
  id: number;
  path: string;
  folder: boolean;
}
export interface MatchPassage {
  snippet: string;
  passage: string;
  digest: string;
  start: number;
  end: number;
}
export interface FileRecord {
  search_match?: MatchPassage;
  id: number;
  root_id: number;
  path: string;
  name: string;
  extension: string;
  category: Category;
  size: number;
  modified: number;
  created: number | null;
  source: string | null;
  screenshot: boolean;
  hash: string | null;
  modified_token?: string;
}
export interface CategoryStat {
  category: Category;
  count: number;
  bytes: number;
}
export interface SourceStat {
  source: string;
  count: number;
  bytes: number;
}
export interface LibraryStats {
  count: number;
  bytes: number;
  categories: CategoryStat[];
  sources: SourceStat[];
  large: number;
  screenshots: number;
  downloads: number;
  duplicate_groups: number;
  duplicate_bytes: number;
}
export interface Job {
  revision: number;
  watch_error: string;
  running: boolean;
  kind: "scan" | "duplicates" | "watch" | "update" | "content" | "";
  processed: number;
  errors: number;
  message: string;
  cancelled: boolean;
  duration_ms: number;
}
export interface Bootstrap {
  roots: Root[];
  stats: LibraryStats;
  job: Job;
  platform: string;
  ocr: string;
  version: string;
}
export interface Query {
  favourites?: boolean;
  category?: string;
  terms: string[];
  min_size?: number;
  max_size?: number;
  date_from?: number;
  date_to?: number;
  screenshot?: boolean;
  source?: string;
  root_id?: number;
  cleanup?: string;
  sort: string;
  offset: number;
  limit: number;
}
export interface QueryResult {
  files: FileRecord[];
  total: number;
  bytes: number;
}
export interface ParsedSearch {
  query: Partial<Query>;
  chips: string[];
  terms: string[];
}
export interface DuplicateGroup {
  hash: string;
  size: number;
  files: FileRecord[];
}
export interface TrashResult {
  trashed: number[];
  failed: { id: number; reason: string }[];
}
