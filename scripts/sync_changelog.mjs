import { execFileSync } from "node:child_process";
import { readFileSync, writeFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const sourcePath = resolve(root, process.env.KOTOCLIP_CHANGELOG_SOURCE ?? "src/version/changelog.source.json");
const outputPath = resolve(root, process.env.KOTOCLIP_CHANGELOG_OUTPUT ?? "src/version/changelog.json");
const refreshVersion = process.env.KOTOCLIP_REFRESH_VERSION ?? "";

const existingReleases = new Map();
try {
  const existingDocument = JSON.parse(readFileSync(outputPath, "utf8"));
  for (const release of existingDocument.releases ?? []) existingReleases.set(release.version, release);
} catch (error) {
  if (error.code !== "ENOENT") throw error;
}

const document = JSON.parse(readFileSync(sourcePath, "utf8"));
const head = execFileSync("git", ["-C", root, "rev-parse", "HEAD"], { encoding: "utf8" }).trim();
for (const release of document.releases ?? []) {
  if (release.source.kind === "commit" && release.source.ref === "HEAD") {
    const existingRelease = existingReleases.get(release.version);
    const existingRef = existingRelease?.source?.ref;
    const canReuseExistingRef = refreshVersion !== release.version
      && existingRelease?.source?.kind === "commit"
      && typeof existingRef === "string"
      && /^[0-9a-f]{40}$/i.test(existingRef);
    release.source.ref = canReuseExistingRef ? existingRef : head;
  }
  release.source.url = release.source.kind === "commit"
    ? `${document.repositoryUrl}/commit/${release.source.ref}`
    : `${document.repositoryUrl}/releases/tag/v${release.version}`;
}

document.latestVersion = document.releases?.[0]?.version;
writeFileSync(outputPath, `${JSON.stringify(document, null, 2)}\n`, "utf8");
