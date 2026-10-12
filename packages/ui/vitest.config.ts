import { defineConfig } from 'vitest/config';

// CSS is processed (not stubbed) so the token tests can read the real stylesheet text with ?raw.
export default defineConfig({ test: { css: { include: /.+/ } } });
