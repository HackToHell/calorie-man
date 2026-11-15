import { fileURLToPath } from 'node:url';
import { defineConfig, type Plugin } from 'vite';
import { svelte } from '@sveltejs/vite-plugin-svelte';

const backend = process.env.CALORIE_MAN_BACKEND ?? 'http://127.0.0.1:8484';

/** `--mode demo`: boot the WebAssembly backend before the app, with a placeholder while it loads. */
const demoEntry: Plugin = {
  name: 'calorie-man-demo-entry',
  transformIndexHtml: {
    order: 'pre',
    handler: (html) =>
      html
        .replace('/src/main.ts', '/src/demo/main.ts')
        .replace('<title>Calorie Man</title>', '<title>Calorie Man · live demo</title>')
        .replace(
          '<div id="app"></div>',
          '<div id="app"><p class="boot">Starting the demo…</p></div>' +
            '<noscript><p class="boot">The Calorie Man demo needs JavaScript.</p></noscript>',
        ),
  },
};

export default defineConfig(({ mode }) => {
  const demo = mode === 'demo';
  return {
    // The demo is served from a subpath (github.io/calorie-man/); routing is hash-based, so relative
    // asset URLs are all it needs.
    base: demo ? './' : '/',
    plugins: [svelte(), demo && demoEntry],
    resolve: {
      alias: { 'calorie-man-wasm': fileURLToPath(new URL('./src/demo/pkg/backend.js', import.meta.url)) },
    },
    server: {
      host: true,
      proxy: demo ? undefined : { '/api': backend, '/mcp': backend },
    },
    build: {
      outDir: demo ? 'dist-demo' : 'dist',
      emptyOutDir: true,
      target: 'es2022',
      chunkSizeWarningLimit: 400,
    },
  };
});
