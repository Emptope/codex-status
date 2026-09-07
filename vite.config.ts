import { defineConfig } from 'vite';
import { svelte } from '@sveltejs/vite-plugin-svelte';
import { buildLayout } from './scripts/build/layout.mjs';
import { preview } from './scripts/build/preview.mjs';

export default defineConfig({
  plugins: [svelte(), preview()],
  clearScreen: false,
  cacheDir: buildLayout.viteCache,
  server: {
    host: '127.0.0.1',
    port: 1420,
    strictPort: true,
    watch: { ignored: [`**/${buildLayout.root}/**`] },
  },
  build: { outDir: buildLayout.webStaging, emptyOutDir: false, target: 'es2022' },
});
