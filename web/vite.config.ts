import { defineConfig } from 'vite';
import { svelte } from '@sveltejs/vite-plugin-svelte';
export default defineConfig({
  plugins: [svelte()],
  server: {
    proxy: {
      '/api': 'http://localhost:8765',
      '/auth': 'http://localhost:8765',
      '/logout': 'http://localhost:8765',
    },
  },
});
