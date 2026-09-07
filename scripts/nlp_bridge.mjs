import { spawn } from 'node:child_process';
import { createInterface } from 'node:readline';
import { resolve } from 'node:path';

// 本地验收服务器复用一个 Rust 进程，按顺序传递请求及响应。
export function nlpBridge() {
  return {
    name: 'kotoclip-nlp-bridge',
    configureServer(server) {
      const executable = resolve('target/debug/kotoclip-nlp.exe');
      const child = spawn(executable, ['stdio'], { windowsHide: true, stdio: ['pipe', 'pipe', 'pipe'] });
      const pending = [];
      let failed;
      function stop(error) {
        failed = error;
        for (const response of pending.splice(0)) {
          response.statusCode = 503;
          response.end(JSON.stringify({ error }));
        }
      }
      child.on('error', error => stop(`分析服务启动失败：${error.message}`));
      child.on('exit', () => stop('分析服务已退出，请重启验收入口'));
      child.stderr.on('data', data => process.stderr.write(data));
      createInterface({ input: child.stdout }).on('line', line => {
        const response = pending.shift();
        if (response) response.end(line);
      });
      server.httpServer?.on('close', () => child.kill());
      server.middlewares.use('/api/nlp', (request, response) => {
        response.setHeader('Content-Type', 'application/json; charset=utf-8');
        if (request.method !== 'POST') { response.statusCode = 405; response.end('{}'); return; }
        const origin = request.headers.origin;
        if (origin) {
          try { if (new URL(origin).host !== request.headers.host) throw new Error(); }
          catch { response.statusCode = 403; response.end('{}'); return; }
        }
        let body = '';
        request.setEncoding('utf8');
        request.on('data', chunk => {
          body += chunk.toString('utf8');
          if (body.length > 1000000) { response.statusCode = 413; response.end('{}'); request.destroy(); }
        });
        request.on('end', () => {
          if (response.writableEnded) return;
          if (failed || pending.length >= 8) { response.statusCode = 503; response.end(JSON.stringify({ error: failed || '请求过多，请稍后重试' })); return; }
          try {
            const value = JSON.parse(body);
            pending.push(response);
            child.stdin.write(JSON.stringify(value) + '\n');
          } catch { response.statusCode = 400; response.end(JSON.stringify({ error: '请求格式错误' })); }
        });
      });
    },
  };
}
