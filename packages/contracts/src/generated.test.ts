import { readdirSync, readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';
import type { ErrorCode } from './generated/ErrorCode';
import type { EventEnvelope } from './generated/EventEnvelope';
import type { EventKind } from './generated/EventKind';
import type { VelaError } from './generated/VelaError';

const generatedDir = new URL('./generated/', import.meta.url);

function generatedFiles(): string[] {
  return readdirSync(generatedDir).filter((name) => name.endsWith('.ts'));
}

/** The string literals of a generated union type such as `export type EventKind = "A" | "B";`. */
function unionMembers(file: string): string[] {
  const source = readFileSync(new URL(file, generatedDir), 'utf8');
  return [...source.matchAll(/"([^"]+)"/g)].map((match) => match[1] ?? '');
}

describe('generated contracts', () => {
  // These literals only compile while the generated types have the shapes the Rust side promises, so `tsc` (the
  // `typecheck` script) is the integration test that the generated TypeScript compiles and is usable.
  it('accepts a well-formed envelope and a typed error', () => {
    const envelope = {
      seq: 1,
      ts: 1_700_000_000_000,
      run_id: 'run-1',
      ticket_id: 'F02',
      worker_id: null,
      kind: 'RunPaused',
      payload: { reason: 'stop requested' },
      schema_version: 1,
    } satisfies EventEnvelope;
    const error = {
      class: 'POLICY_BLOCK',
      code: 'PROVIDER_POLICY_BLOCK',
      origin: 'provider',
      what_failed: 'blocked',
      state_preserved: '',
      side_effects: { committed: false, pushed: false, merged: false },
      retry: 'never',
      user_action: '',
    } satisfies VelaError;
    const code: ErrorCode = error.code;
    expect(envelope.kind).toBe('RunPaused');
    expect(code).toBe('PROVIDER_POLICY_BLOCK');
  });

  it('has exactly one payload type per event kind', () => {
    const kinds: EventKind[] = unionMembers('EventKind.ts') as EventKind[];
    expect(kinds).toHaveLength(53);
    const payloadFiles = generatedFiles()
      .filter((name) => name.endsWith('Payload.ts'))
      .map((name) => name.replace(/Payload\.ts$/, ''))
      .sort();
    expect(payloadFiles).toEqual([...kinds].sort());
  });

  it('is reachable through the package wildcard export only', () => {
    const pkg = JSON.parse(readFileSync(new URL('../package.json', import.meta.url), 'utf8')) as {
      exports: Record<string, string>;
    };
    expect(pkg.exports['./generated/*']).toBe('./src/generated/*.ts');
    expect(generatedFiles()).not.toContain('index.ts');
  });
});
