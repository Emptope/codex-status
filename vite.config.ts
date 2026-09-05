import { defineConfig } from 'vite';
import { svelte } from '@sveltejs/vite-plugin-svelte';
import { preview } from './scripts/build/preview.mjs';

export default defineConfig({
  plugins: [svelte(), preview()],
  clearScreen: false,
  cacheDir: 'build/vite-cache',
  server: {
    host: '127.0.0.1',
    port: 1420,
    strictPort: true,
    watch: { ignored: ['**/build/**'] },
  },
  build: { outDir: 'build/web', emptyOutDir: false, target: 'es2022' },
});
