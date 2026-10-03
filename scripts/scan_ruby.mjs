import { spawn } from 'node:child_process';
import { createInterface } from 'node:readline';
import { readFile, writeFile } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';

const [source, output] = process.argv.slice(2);
if (!source || !output) throw new Error('用法：node scripts/scan_ruby.mjs 正文路径 报告路径');
const root = fileURLToPath(new URL('../', import.meta.url));
const executable = fileURLToPath(new URL(`../target/debug/kotoclip-nlp${process.platform === 'win32' ? '.exe' : ''}`, import.meta.url));
const lines = (await readFile(source, 'utf8')).split(/\r?\n/);
const child = spawn(executable, ['stdio'], { cwd: root, stdio: ['pipe', 'pipe', 'inherit'], windowsHide: true });
const responses = createInterface({ input: child.stdout });
let pending;
let failure;
const fail = error => { failure = error; pending?.reject(error); pending = undefined; };
child.on('error', fail);
child.stdin.on('error', fail);
const closed = new Promise(resolve => child.once('close', code => {
  if (pending || code !== 0) fail(new Error(`分析进程退出：${code}`));
  resolve(code);
}));
responses.on('line', line => {
  const request = pending;
  pending = undefined;
  try { request?.resolve(JSON.parse(line)); } catch (error) { request?.reject(error); }
});
function request(value) {
  if (failure) return Promise.reject(failure);
  return new Promise((resolve, reject) => {
    pending = { resolve, reject };
    child.stdin.write(JSON.stringify(value) + '\n', 'utf8');
  });
}

const rows = [];
let analyzedLines = 0;
try {
  for (const [index, line] of lines.entries()) {
    if (!line.includes('《') || !line.includes('》')) continue;
    const response = await request({ command: 'analyze', text: line, register: 'cwj' });
    if (response.error) throw new Error(`第 ${index + 1} 行：${response.error}`);
    analyzedLines += 1;
    for (const validation of response.result.ruby_validations) {
      rows.push({ line: index + 1, ...validation, provider: response.result.source.provider.id, source: line });
    }
  }
} finally {
  child.stdin.end();
  await closed;
  responses.close();
}
if (failure) throw failure;
const counts = { matched: 0, variant: 0, unavailable: 0, unmatched: 0 };
const reasons = {};
for (const row of rows) {
  counts[row.status] += 1;
  reasons[row.reason] = (reasons[row.reason] || 0) + 1;
}
const summary = { source, lines: lines.length, analyzed_lines: analyzedLines, ruby: rows.length, counts, reasons };
await writeFile(output, JSON.stringify({ ...summary, rows }, null, 2) + '\n', 'utf8');
console.log(JSON.stringify(summary, null, 2));
