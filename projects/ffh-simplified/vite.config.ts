import { defineConfig } from 'vite';
import { resolve } from 'path';

export default defineConfig({
  resolve: {
    alias: {
      '@': resolve(__dirname, 'src'),
    },
  },
  server: {
    port: 3000,
    host: true, // Listen on all interfaces (needed for WSL)
    watch: {
      usePolling: true, // Required for WSL file watching
      interval: 1000,
    },
  },
});
