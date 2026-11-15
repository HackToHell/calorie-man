// Stand-in for USDA FoodData Central and Open Food Facts, serving real saved responses from
// ../tests/fixtures. Started by Playwright's global setup.
import { createServer } from 'node:http';
import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const dir = join(dirname(fileURLToPath(import.meta.url)), '..', '..', 'tests', 'fixtures');
const fx = (n) => readFileSync(join(dir, n), 'utf8');

export function startMock(port) {
  const server = createServer((req, res) => {
    const url = new URL(req.url, 'http://x');
    const send = (code, body) => {
      res.writeHead(code, { 'content-type': 'application/json' });
      res.end(body);
    };
    const p = url.pathname;
    if (p === '/fdc/foods/search') {
      const q = url.searchParams.get('query') ?? '';
      if (q.includes('ratelimit')) return send(429, '{}');
      if (/^\d+$/.test(q)) {
        // UPC search: return the Kirkland milk list, which contains 096619163014
        return send(200, fx('usda_search_kirkland_milk.json'));
      }
      if (q.includes('banana')) return send(200, fx('usda_search_banana.json'));
      if (q.includes('milk') || q.includes('kirkland')) return send(200, fx('usda_search_kirkland_milk.json'));
      return send(200, JSON.stringify({ totalHits: 0, foods: [] }));
    }
    let m = p.match(/^\/fdc\/food\/(\d+)$/);
    if (m) {
      if (m[1] === '2460407') return send(200, fx('usda_food_2460407.json'));
      if (m[1] === '173944') return send(200, fx('usda_food_173944.json'));
      return send(404, '{}');
    }
    m = p.match(/^\/off\/api\/v2\/product\/(\d+)\.json$/);
    if (m) {
      if (m[1] === '3017624010701') return send(200, fx('off_product_nutella.json'));
      if (m[1].replace(/^0+/, '') === '96619204205') return send(200, fx('off_product_kirkland.json'));
      return send(200, JSON.stringify({ status: 0, status_verbose: 'product not found' }));
    }
    if (p === '/offsearch/search') return send(200, fx('off_search_kirkland_milk.json'));
    send(404, '{"error":"mock: no route"}');
  });
  return new Promise((resolve) => server.listen(port, '127.0.0.1', () => resolve(server)));
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const port = Number(process.argv[2] ?? 18601);
  await startMock(port);
  console.log(`mock external APIs on http://127.0.0.1:${port}`);
}
