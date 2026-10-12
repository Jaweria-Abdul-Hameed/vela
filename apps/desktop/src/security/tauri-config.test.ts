import { readdirSync, readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';
import { wireName } from '@vela/contracts/commands/diagnostics';

// Every command group wrapper in packages/contracts/src/commands, discovered so a new group needs no edit here.
const groupModules = import.meta.glob<{ commandNames: readonly string[] }>(
  [
    '../../../../packages/contracts/src/commands/*.ts',
    '!../../../../packages/contracts/src/commands/*.test.ts',
  ],
  { eager: true },
);
const groups = Object.entries(groupModules)
  .map(([path, module]) => ({
    group: /([^/]+)\.ts$/.exec(path)?.[1] ?? '',
    names: [...module.commandNames],
  }))
  .sort((a, b) => a.group.localeCompare(b.group));
const commandNames = groups.flatMap((g) => g.names);

const shell = new URL('../../src-tauri/', import.meta.url);
const readJson = (path: string): unknown => JSON.parse(readFileSync(new URL(path, shell), 'utf8'));

interface Config {
  build: { frontendDist: string };
  app: {
    security: { csp: Record<string, string>; freezePrototype: boolean };
    windows: Array<{ url?: string }>;
  };
  bundle: Record<string, unknown>;
  plugins: Record<string, unknown>;
}
interface Capability {
  windows?: string[];
  webviews?: string[];
  remote?: unknown;
  permissions: string[];
}

const config = readJson('tauri.conf.json') as Config;
const csp = config.app.security.csp;

// A deliberately small model of CSP source matching, enough to decide whether a URL may load for a directive:
// the effective directive falls back to default-src; 'self' matches the app's own origin; a scheme or host source
// matches only itself. 'none' matches nothing.
const APP_ORIGINS = ['http://tauri.localhost', 'tauri://localhost'];
function allows(directive: string, url: string): boolean {
  const policy = csp[directive] ?? csp['default-src'] ?? '';
  const origin = new URL(url).origin;
  return policy.split(/\s+/).some((source) => {
    if (source === "'none'" || source === '') return false;
    if (source === "'self'") return APP_ORIGINS.includes(origin);
    if (source.endsWith(':')) return new URL(url).protocol === source;
    return origin === source || url.startsWith(source);
  });
}

describe('content security policy', () => {
  it('blocks a script, style, image, font, frame and connection from a remote origin', () => {
    const remote = 'https://evil.example/payload.js';
    for (const directive of [
      'script-src',
      'style-src',
      'img-src',
      'font-src',
      'frame-src',
      'connect-src',
      'object-src',
      'worker-src',
    ]) {
      expect(allows(directive, remote), directive).toBe(false);
    }
    expect(allows('script-src', 'http://evil.example/x.js')).toBe(false);
    expect(allows('connect-src', 'wss://evil.example/socket')).toBe(false);
  });

  it('still allows the shell its own assets and the IPC channel', () => {
    expect(allows('script-src', 'http://tauri.localhost/assets/index.js')).toBe(true);
    expect(allows('style-src', 'http://tauri.localhost/assets/index.css')).toBe(true);
    expect(allows('connect-src', 'http://ipc.localhost/diagnostics_ping')).toBe(true);
  });

  it('denies by default and has no wildcard, remote host, eval or inline allowance', () => {
    expect(csp['default-src']).toBe("'none'");
    for (const [directive, value] of Object.entries(csp)) {
      expect(value, directive).not.toMatch(
        /\*|https?:\/\/(?!ipc\.localhost)|unsafe-|blob:|filesystem:/,
      );
    }
    expect(csp['object-src']).toBe("'none'");
    expect(csp['base-uri']).toBe("'none'");
    expect(config.app.security.freezePrototype).toBe(true);
  });

  it('loads the window from the bundled assets, never a remote URL', () => {
    for (const window of config.app.windows) expect(window.url).toBeUndefined();
    expect(config.build.frontendDist).toBe('../dist');
  });

  it('leaves bundle and plugins empty for the tickets that own them', () => {
    expect(config.bundle).toEqual({});
    expect(config.plugins).toEqual({});
  });
});

describe('capabilities', () => {
  const dir = new URL('capabilities/', shell);
  const files = readdirSync(dir).filter((name) => name.endsWith('.json'));
  const capabilities = files.map((name) => readJson(`capabilities/${name}`) as Capability);

  it('are one JSON file per surface and grant the main window only', () => {
    expect(files).toEqual(['default.json']);
    for (const capability of capabilities) {
      expect(capability.windows).toEqual(['main']);
      expect(capability.remote).toBeUndefined();
      expect(capability.webviews).toBeUndefined();
    }
  });

  it('allow exactly the registered commands plus event listening', () => {
    const granted = capabilities.flatMap((c) => c.permissions).sort();
    const commands = commandNames.map((name) => `allow-${wireName(name).replaceAll('_', '-')}`);
    expect(granted).toEqual(
      [...commands, 'core:event:allow-listen', 'core:event:allow-unlisten'].sort(),
    );
  });

  it('never grant filesystem, shell, http or opener access', () => {
    for (const permission of capabilities.flatMap((c) => c.permissions)) {
      expect(permission).not.toMatch(/^(fs|shell|http|opener|process|os|dialog|clipboard)/);
      expect(permission).not.toMatch(/:default$/);
    }
  });

  it('are backed by one permission file per command group that allows only registered commands', () => {
    const permissions = readdirSync(new URL('permissions/', shell)).filter((n) =>
      n.endsWith('.toml'),
    );
    expect(permissions).toEqual(groups.map((g) => `${g.group}.toml`));
    for (const { group, names } of groups) {
      const toml = readFileSync(new URL(`permissions/${group}.toml`, shell), 'utf8');
      expect([...toml.matchAll(/commands\.allow = \["([^"]+)"\]/g)].map((m) => m[1])).toEqual(
        names.map(wireName),
      );
    }
  });
});
