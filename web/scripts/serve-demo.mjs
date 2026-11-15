// Serve the demo build the way GitHub Pages does: static files under /calorie-man/.
// Usage: node scripts/serve-demo.mjs [--port 8686] [--dir dist-demo]
import { createReadStream, statSync } from 'node:fs';
import { createServer } from 'node:http';
import { extname, join, normalize, resolve } from 'node:path';

const arg = (name, def) => {
  const i = process.argv.indexOf(`--${name}`);
  return i > 0 ? process.argv[i + 1] : def;
};
const port = Number(arg('port', 8686));
const dir = resolve(arg('dir', 'dist-demo'));
const prefix = '/calorie-man/';
const types = {
  '.html': 'text/html; charset=utf-8',
  '.js': 'text/javascript',
  '.css': 'text/css',
  '.wasm': 'application/wasm',
  '.svg': 'image/svg+xml',
  '.woff2': 'font/woff2',
  '.webmanifest': 'application/manifest+json',
  '.txt': 'text/plain',
};

createServer((req, res) => {
  const path = decodeURIComponent(new URL(req.url, 'http://x').pathname);
  if (path === '/' || path === '/calorie-man') {
    res.writeHead(302, { location: prefix }).end();
    return;
  }
  if (!path.startsWith(prefix)) {
    res.writeHead(404).end('not found');
    return;
  }
  let file = normalize(join(dir, path.slice(prefix.length)));
  if (!file.startsWith(dir)) {
    res.writeHead(403).end();
    return;
  }
  try {
    if (statSync(file).isDirectory()) file = join(file, 'index.html');
    statSync(file);
  } catch {
    res.writeHead(404).end('not found');
    return;
  }
  res.writeHead(200, { 'content-type': types[extname(file)] ?? 'application/octet-stream' });
  createReadStream(file).pipe(res);
}).listen(port, '127.0.0.1', () => console.log(`demo at http://127.0.0.1:${port}${prefix}`));
