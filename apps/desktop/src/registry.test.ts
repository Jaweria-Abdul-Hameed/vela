import { describe, expect, it } from 'vitest';
import { collectScenes, collectSurfaces, scenes, surfaces } from './registry';

describe('scene and surface registration', () => {
  it('a dummy scene file and a dummy surface file appear without editing any existing file', () => {
    const sceneModules = import.meta.glob('./__fixtures__/scenes/*.tsx', { eager: true });
    const surfaceModules = import.meta.glob('./__fixtures__/surfaces/*/index.ts', { eager: true });
    expect(collectScenes(sceneModules).map((s) => s.id)).toEqual(['dummy-scene']);
    expect(collectSurfaces(surfaceModules).map((s) => s.id)).toEqual(['dummy-surface']);
  });

  it('orders scenes by order then id and skips modules that register nothing', () => {
    const c = () => null;
    const found = collectScenes({
      './scenes/b.tsx': { scene: { id: 'b', title: 'B', component: c } },
      './scenes/a.tsx': { scene: { id: 'a', title: 'A', order: 1, component: c } },
      './scenes/none.tsx': {},
      './scenes/c.tsx': { scene: { id: 'c', title: 'C', component: c } },
    });
    expect(found.map((s) => s.id)).toEqual(['a', 'b', 'c']);
  });

  it('the real registries are built from the skeleton modules without error', () => {
    expect(Array.isArray(scenes)).toBe(true);
    expect(Array.isArray(surfaces)).toBe(true);
  });
});
