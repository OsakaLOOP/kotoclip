export interface LibraryBookSummary {
  id: string;
  title: string;
  author: string;
  language: string;
  sourceName: string;
  coverPath?: string | null;
  chapterCount: number;
  totalCharacters: number;
  progressOffset: number;
  progressPercent: number;
  currentChapter?: string | null;
  createdAt: string;
  lastOpenedAt?: string | null;
}

export interface LibraryResource {
  href: string;
  path: string;
  mediaType: string;
}

export interface LibraryBook extends LibraryBookSummary {
  markdown: string;
  chapterTitles: string[];
  resources: LibraryResource[];
  warnings: string[];
  libraryPath: string;
}

export function resourceKey(path: string): string {
  const clean = path.split(/[?#]/, 1)[0].replace(/\\/g, "/");
  return decodeURIComponent(clean.slice(clean.lastIndexOf("/") + 1)).toLocaleLowerCase();
}

export function resourcePathMap(resources: LibraryResource[]): Map<string, string> {
  return new Map(resources.map((resource) => [resourceKey(resource.href), resource.path]));
}
