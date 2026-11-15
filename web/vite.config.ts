import { defineConfig } from 'vite';
import { svelte } from '@sveltejs/vite-plugin-svelte';

const backend = process.env.CALORIE_MAN_BACKEND ?? 'http://127.0.0.1:8484';

export default defineConfig({
  plugins: [svelte()],
  server: {
    host: true,
    proxy: { '/api': backend, '/mcp': backend },
  },
  build: {
    outDir: 'dist',
    emptyOutDir: true,
    target: 'es2022',
    chunkSizeWarningLimit: 400,
  },
});
