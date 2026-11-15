// The demo has no server: requests to /api and /mcp are answered by the real Rust backend compiled to
// WebAssembly, running against an in-memory SQLite database that starts over on every page load.
import init, { handle, start } from 'calorie-man-wasm';

const BACKEND_PATH = /^\/(api\/|mcp$)/;

function pairs(flat: string[]): [string, string][] {
  const out: [string, string][] = [];
  for (let i = 0; i + 1 < flat.length; i += 2) out.push([flat[i], flat[i + 1]]);
  return out;
}

export async function startDemoBackend(days = 60) {
  await init();
  start(days);

  const networkFetch = window.fetch.bind(window);
  window.fetch = async (input: RequestInfo | URL, options?: RequestInit) => {
    const req = new Request(input, options);
    const url = new URL(req.url);
    if (url.origin !== location.origin || !BACKEND_PATH.test(url.pathname)) return networkFetch(input, options);
    const body = req.method === 'GET' || req.method === 'HEAD' ? null : new Uint8Array(await req.arrayBuffer());
    const headers: string[] = [];
    req.headers.forEach((v, k) => headers.push(k, v));
    const out = await handle(req.method, url.pathname + url.search, headers, body);
    if (req.signal.aborted) throw new DOMException('The request was aborted.', 'AbortError');
    return new Response(out.body as BodyInit | null, { status: out.status, headers: pairs(out.headers) });
  };

  // Export links are plain <a href="/api/..." download>; there is no server to follow them, so fetch the
  // file through the shim and save it from a blob instead.
  document.addEventListener('click', async (e) => {
    const a = (e.target as Element | null)?.closest?.('a[href^="/api/"]') as HTMLAnchorElement | null;
    if (!a || e.defaultPrevented) return;
    e.preventDefault();
    const res = await window.fetch(a.getAttribute('href')!);
    if (!res.ok) {
      const msg = ((await res.json().catch(() => null)) as { error?: string } | null)?.error ?? `HTTP ${res.status}`;
      const { app } = await import('../lib/state.svelte');
      app.toast(msg, 'error');
      return;
    }
    const name = /filename="([^"]+)"/.exec(res.headers.get('content-disposition') ?? '')?.[1] ?? 'calorie-man-export';
    const href = URL.createObjectURL(await res.blob());
    const link = Object.assign(document.createElement('a'), { href, download: name });
    link.click();
    setTimeout(() => URL.revokeObjectURL(href), 10_000);
  });
}
