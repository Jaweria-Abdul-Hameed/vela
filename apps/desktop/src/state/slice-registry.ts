// Slices register themselves: every module in `slices/` that exports `slice` is picked up here, so a feature ticket adds a
// file and edits nothing else (SHARED_SURFACE_PROTOCOL.md section 7).
import type { Slice } from './reducer';

/** Slices from a module map (the result of `import.meta.glob`); modules without a `slice` export are skipped. */
export function collectSlices(modules: Record<string, unknown>): Slice[] {
  return Object.keys(modules)
    .sort()
    .flatMap((path) => {
      const candidate = (modules[path] as { slice?: Slice } | undefined)?.slice;
      return candidate === undefined ? [] : [candidate];
    });
}

export const registeredSlices: Slice[] = collectSlices(
  import.meta.glob('./slices/*.ts', { eager: true }),
);
