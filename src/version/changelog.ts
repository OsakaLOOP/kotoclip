import sourceChangelog from "./changelog.json";

export type ReleaseType = "feat" | "fix" | "security" | "next";
export type ReleaseSourceKind = "commit" | "release";

export interface ReleaseSource {
  kind: ReleaseSourceKind;
  ref: string;
  url: string;
}

export interface ChangelogRelease {
  version: string;
  date: string;
  type: ReleaseType;
  summary: string;
  details: string[];
  source: ReleaseSource;
}

export interface ChangelogDocument {
  schemaVersion: number;
  repositoryUrl: string;
  currentVersion: string;
  latestVersion: string;
  lastUpdated: string;
  updateCheck: {
    enabled: boolean;
    provider: string | null;
  };
  releases: ChangelogRelease[];
}

export const changelog = sourceChangelog as ChangelogDocument;
export const CURRENT_VERSION = changelog.currentVersion;

export interface UpdateCheckProvider {
  check(currentVersion: string): Promise<ChangelogRelease | null>;
}

/** 在线更新检查的扩展入口；默认不提供 provider，因此不会发起网络请求。 */
export async function checkForUpdates(provider?: UpdateCheckProvider): Promise<ChangelogRelease | null> {
  return provider ? provider.check(CURRENT_VERSION) : null;
}
