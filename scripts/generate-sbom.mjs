// Software bill of materials (X05): a CycloneDX 1.5 JSON document built from the locked dependency graphs.
// Rust components come from `cargo metadata --locked`, npm components from package-lock.json.
// Usage: node scripts/generate-sbom.mjs [output-path]   (default target/sbom.cdx.json)
import { execFileSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.join(path.dirname(fileURLToPath(import.meta.url)), '..');

/** Components for the external (registry) crates in `cargo metadata` output; workspace members are not components. */
export function rustComponents(metadata) {
  return metadata.packages
    .filter((p) => p.source !== null && p.source !== undefined)
    .map((p) => ({
      type: 'library',
      name: p.name,
      version: p.version,
      purl: `pkg:cargo/${p.name}@${p.version}`,
      licenses: p.license ? [{ expression: p.license }] : [],
    }));
}

/** Components for every installed package in a v2/v3 package-lock.json (the root and workspace links are skipped). */
export function npmComponents(lock) {
  return Object.entries(lock.packages ?? {})
    .filter(([key, p]) => key !== '' && key.includes('node_modules/') && !p.link)
    .map(([key, p]) => {
      const name = key.slice(key.lastIndexOf('node_modules/') + 'node_modules/'.length);
      return {
        type: 'library',
        name,
        version: p.version,
        purl: `pkg:npm/${name.replace('@', '%40')}@${p.version}`,
        licenses: p.license ? [{ expression: String(p.license) }] : [],
      };
    });
}

export function buildSbom(rust, npm, now = new Date()) {
  const components = [...rust, ...npm].sort((a, b) => a.purl.localeCompare(b.purl));
  return {
    bomFormat: 'CycloneDX',
    specVersion: '1.5',
    version: 1,
    metadata: { timestamp: now.toISOString(), component: { type: 'application', name: 'vela' } },
    components,
  };
}

if (
  process.argv[1] &&
  path.resolve(process.argv[1]) === path.resolve(fileURLToPath(import.meta.url))
) {
  const metadata = JSON.parse(
    execFileSync('cargo', ['metadata', '--locked', '--format-version', '1'], {
      cwd: root,
      encoding: 'utf8',
      maxBuffer: 256 * 1024 * 1024,
    }),
  );
  const lock = JSON.parse(fs.readFileSync(path.join(root, 'package-lock.json'), 'utf8'));
  const sbom = buildSbom(rustComponents(metadata), npmComponents(lock));
  const out = process.argv[2] ?? path.join(root, 'target', 'sbom.cdx.json');
  fs.mkdirSync(path.dirname(out), { recursive: true });
  fs.writeFileSync(out, `${JSON.stringify(sbom, null, 2)}\n`);
  console.log(`wrote ${sbom.components.length} components to ${out}`);
}
