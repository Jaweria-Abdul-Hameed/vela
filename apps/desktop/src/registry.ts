// Scenes and surfaces register themselves (F05, SHARED_SURFACE_PROTOCOL.md section 7): a module in `scenes/` exports `scene`,
// a module in `surfaces/<name>/` exports `surface`, and `import.meta.glob` finds them. Adding a file adds an entry;
// no existing file is edited.
import type { ComponentType } from 'react';

export interface SceneRegistration {
  id: string;
  title: string;
  /** Lower first; ties break by id. */
  order?: number;
  component: ComponentType;
}

export interface SurfaceRegistration {
  id: string;
  title: string;
  component: ComponentType;
}

function collect<T extends { id: string }>(
  modules: Record<string, unknown>,
  key: 'scene' | 'surface',
): T[] {
  const found: T[] = [];
  for (const path of Object.keys(modules).sort()) {
    const registration = (modules[path] as Record<string, T | undefined> | undefined)?.[key];
    if (registration !== undefined) found.push(registration);
  }
  return found;
}

export function collectScenes(modules: Record<string, unknown>): SceneRegistration[] {
  return collect<SceneRegistration>(modules, 'scene').sort(
    (a, b) => (a.order ?? 100) - (b.order ?? 100) || a.id.localeCompare(b.id),
  );
}

export function collectSurfaces(modules: Record<string, unknown>): SurfaceRegistration[] {
  return collect<SurfaceRegistration>(modules, 'surface').sort((a, b) => a.id.localeCompare(b.id));
}

export const scenes: SceneRegistration[] = collectScenes(
  import.meta.glob('./scenes/*.tsx', { eager: true }),
);

export const surfaces: SurfaceRegistration[] = collectSurfaces(
  import.meta.glob('./surfaces/*/index.ts', { eager: true }),
);
