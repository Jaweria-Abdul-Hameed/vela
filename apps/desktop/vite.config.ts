import react from '@vitejs/plugin-react';
import { defineConfig } from 'vitest/config';

// The renderer for the Tauri shell. Development serves on a fixed port (tauri.conf.json build.devUrl); the
// production bundle in dist is embedded in the executable and served by the Tauri protocol, so production needs
// no server. modulePreload.polyfill is off so the bundle contains no inline script under the strict CSP.
export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: { port: 1420, strictPort: true },
  build: { outDir: 'dist', target: 'es2023', modulePreload: { polyfill: false } },
});
