import { execFileSync } from "node:child_process";
import { existsSync, readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const configPath = resolve(root, "src-tauri/tauri.github-release.conf.json");
const config = JSON.parse(readFileSync(configPath, "utf8"));
const requiredBundles = [
  "data/dict-sources/starter.kdict",
  "data/dict-sources/daijirin.kdict",
  "data/dict-sources/shogakukan.kdict",
  "data/dict-sources/crown.kdict",
];
const resources = config.bundle?.resources ?? [];

for (const relativePath of requiredBundles) {
  if (!existsSync(resolve(root, relativePath))) throw new Error(`发行资源缺失：${relativePath}`);
  try {
    execFileSync("git", ["-C", root, "ls-files", "--error-unmatch", "--", relativePath], { stdio: "ignore" });
  } catch {
    throw new Error(`发行资源未被 Git 跟踪：${relativePath}`);
  }
  const bundlePath = `../${relativePath}`;
  if (!resources.includes(bundlePath)) throw new Error(`Tauri 配置未声明发行资源：${bundlePath}`);
}

const dicResources = resources.filter((resource) => /\.dic$/i.test(resource));
if (dicResources.length > 0) throw new Error(`Actions 发行配置不应包含 .dic：${dicResources.join(", ")}`);
console.log(`已确认 ${requiredBundles.length} 个 .kdict 资源存在且已被 Git 跟踪。`);
