import { defineConfig } from 'vite';
import solid from 'vite-plugin-solid';

export default defineConfig({
  plugins: [solid()],
  server: {
    host: '127.0.0.1',
    port: 1420,
    strictPort: true,
    // The Wails window proxies the page, so point HMR at Vite directly.
    hmr: { host: '127.0.0.1', port: 1420 },
    watch: { ignored: ['**/build/**'] },
  },
  clearScreen: false,
});
