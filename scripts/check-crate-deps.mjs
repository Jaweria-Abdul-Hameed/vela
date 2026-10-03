// Crate dependency-direction check (F01; IMPLEMENTATION_ARCHITECTURE.md section 3, SHARED_SURFACE_PROTOCOL.md).
//
// Reads `cargo metadata --no-deps` for the workspace and fails on any forbidden edge. The policy below is
// the machine-readable form of the section 3 rules; changing it is a specification change, not a ticket detail.

import { execFileSync } from 'node:child_process';

/** @typedef {{ name: string, kind: string | null, target: string | null, uses_default_features: boolean, features: string[] }} CargoDependency */
/** @typedef {{ name: string, dependencies: CargoDependency[] }} CargoPackage */
/** @typedef {{ packages: CargoPackage[] }} CargoMetadata */

const DOMAIN = 'vela-domain';
const PERSISTENCE = 'vela-persistence';
const PROCESS = 'vela-process';
const GIT = 'vela-git';
const ADAPTERS = 'vela-adapters';
const UIA = 'vela-uia';
const HOOK = 'vela-hook';
const CORE = 'vela-core';
const TESTKIT = 'vela-testkit';
const DESKTOP = 'vela-desktop';

/**
 * Vela crates each production crate may depend on through normal and build dependencies.
 * Rule numbers refer to IMPLEMENTATION_ARCHITECTURE.md section 3.
 * @type {Record<string, string[]>}
 */
export const ALLOWED_VELA_DEPS = {
  [DOMAIN]: [], // rule 1
  [PERSISTENCE]: [DOMAIN],
  [PROCESS]: [DOMAIN],
  [GIT]: [DOMAIN, PROCESS], // rule 2: never vela-core, another adapter, or persistence
  [ADAPTERS]: [DOMAIN, PROCESS], // rule 2
  [UIA]: [DOMAIN, PROCESS], // rule 2
  [HOOK]: [DOMAIN], // rule 5 (feature shape checked separately)
  [CORE]: [DOMAIN, PERSISTENCE], // rule 3
  [TESTKIT]: [DOMAIN, PROCESS], // dev-only crate
  [DESKTOP]: [DOMAIN, PERSISTENCE, PROCESS, GIT, ADAPTERS, UIA, CORE], // rule 4: the composition root
};

/**
 * Test and tool members. They are not production crates; they may use production libraries but are
 * never depended on by anything.
 * @type {Record<string, string[]>}
 */
export const HARNESS_VELA_DEPS = {
  'vela-integration-tests': [DOMAIN, PERSISTENCE, PROCESS, GIT, ADAPTERS, CORE, TESTKIT],
  'vela-e2e-tests': [DOMAIN, PERSISTENCE, PROCESS, GIT, ADAPTERS, CORE, TESTKIT],
  'vela-antigravity-compat': [DOMAIN, PROCESS, ADAPTERS, TESTKIT],
  'fake-approval-window': [],
};

const isTauri = (/** @type {string} */ name) => name === 'tauri' || name.startsWith('tauri-');

/**
 * @param {CargoMetadata} metadata `cargo metadata --no-deps --format-version 1` output
 * @returns {string[]} one human-readable line per violation; empty when the graph is legal
 */
export function checkDependencyDirection(metadata) {
  /** @type {string[]} */
  const violations = [];
  const members = new Set(metadata.packages.map((p) => p.name));

  for (const pkg of metadata.packages) {
    const production = Object.hasOwn(ALLOWED_VELA_DEPS, pkg.name);
    const harness = Object.hasOwn(HARNESS_VELA_DEPS, pkg.name);
    if (!production && !harness) {
      violations.push(
        `[layout] ${pkg.name} is not a known workspace member; the layout is frozen (no speculative or provider crates, ADR-008)`,
      );
      continue;
    }
    const allowed = new Set(production ? ALLOWED_VELA_DEPS[pkg.name] : HARNESS_VELA_DEPS[pkg.name]);

    for (const dep of pkg.dependencies) {
      const isVela = members.has(dep.name);
      const kind = dep.kind ?? 'normal';

      if (isTauri(dep.name) && pkg.name !== DESKTOP) {
        violations.push(
          `[rule 4] ${pkg.name} must not depend on ${dep.name}; only ${DESKTOP} may depend on Tauri`,
        );
      }
      if (!isVela) continue;

      if (dep.name === UIA && !(dep.target ?? '').includes('cfg(windows)')) {
        violations.push(`[rule 6] ${pkg.name} must depend on ${UIA} only under cfg(windows)`);
      }

      if (dep.name === HOOK || dep.name === DESKTOP) {
        violations.push(
          `[rule 4/5] ${pkg.name} must not depend on ${dep.name} (a binary / the composition root)`,
        );
        continue;
      }
      if (pkg.name === DOMAIN) {
        violations.push(
          `[rule 1] ${DOMAIN} must not depend on another Vela crate (found ${dep.name}, ${kind})`,
        );
        continue;
      }
      if (kind === 'dev') {
        // Dev-dependencies may add test support (vela-testkit) but never an edge the production policy forbids,
        // except that vela-testkit itself is allowed for any crate except vela-domain (handled above).
        // vela-testkit depends on the crates in its own allow list; using it as a dev-dependency of one of them
        // would build two copies of that crate in `cargo test` (cargo permits the cycle, the types then mismatch).
        if (dep.name === TESTKIT && ALLOWED_VELA_DEPS[TESTKIT]?.includes(pkg.name)) {
          violations.push(
            `[dev] ${pkg.name} must not use ${TESTKIT} as a dev-dependency (testkit depends on it)`,
          );
          continue;
        }
        if (dep.name === TESTKIT || allowed.has(dep.name)) continue;
        violations.push(`[dev] ${pkg.name} must not use ${dep.name} as a dev-dependency`);
        continue;
      }
      if (!allowed.has(dep.name)) {
        violations.push(`[rule 1-3] ${pkg.name} must not depend on ${dep.name} (${kind})`);
      }
    }

    if (pkg.name === HOOK) checkHook(pkg, violations);
    if (pkg.name === UIA) checkWindowsOnly(pkg, violations);
  }
  return violations;
}

/** Rule 5: the hook binary depends only on serde (and serde_json) plus the `hook-table` slice of vela-domain. */
function checkHook(/** @type {CargoPackage} */ pkg, /** @type {string[]} */ violations) {
  const thirdParty = new Set(['serde', 'serde_json']);
  for (const dep of pkg.dependencies) {
    if (dep.kind === 'dev') continue;
    if (dep.name === DOMAIN) {
      const shaped =
        dep.uses_default_features === false &&
        dep.features.length === 1 &&
        dep.features[0] === 'hook-table';
      if (!shaped) {
        violations.push(
          `[rule 5] ${HOOK} must depend on ${DOMAIN} with default-features = false and features = ["hook-table"] only`,
        );
      }
    } else if (!thirdParty.has(dep.name)) {
      violations.push(
        `[rule 5] ${HOOK} must not depend on ${dep.name}; only serde, serde_json and the rule-table types`,
      );
    }
  }
}

/** Rule 6: vela-uia compiles only on Windows, so its `windows` dependency must be cfg(windows)-gated. */
function checkWindowsOnly(/** @type {CargoPackage} */ pkg, /** @type {string[]} */ violations) {
  for (const dep of pkg.dependencies) {
    if (dep.name === 'windows' && !(dep.target ?? '').includes('cfg(windows)')) {
      violations.push(`[rule 6] ${UIA} must depend on windows only under cfg(windows)`);
    }
  }
}

/** Crates that must never appear in the resolved dependency tree of the hook binary (rule 5). */
const HOOK_FORBIDDEN = ['petgraph', 'ts-rs', 'rusqlite', 'libsqlite3-sys', 'wry'];

/**
 * Rule 5 against the resolved tree, not just the manifest shape: `cargo tree -p vela-hook --prefix none`
 * output must contain no graph, SQLite, contract-generation or Tauri crate.
 * @param {string} treeOutput
 * @returns {string[]}
 */
export function checkHookTree(treeOutput) {
  /** @type {Set<string>} */
  const found = new Set();
  for (const line of treeOutput.split(/\r?\n/)) {
    const name = line.trim().split(' ')[0] ?? '';
    if (HOOK_FORBIDDEN.includes(name) || isTauri(name)) found.add(name);
  }
  return [...found].map(
    (name) =>
      `[rule 5] the resolved dependency tree of ${HOOK} contains ${name} (no graph, SQLite, or Tauri code allowed)`,
  );
}

/** @param {string[]} args */
function main(args) {
  const manifestFlag = args.indexOf('--manifest-path');
  const cargoArgs = ['metadata', '--format-version', '1', '--no-deps'];
  if (manifestFlag !== -1) cargoArgs.push('--manifest-path', args[manifestFlag + 1] ?? '');
  /** @type {CargoMetadata} */
  const metadata = JSON.parse(
    execFileSync('cargo', cargoArgs, { encoding: 'utf8', maxBuffer: 64 * 1024 * 1024 }),
  );
  const violations = checkDependencyDirection(metadata);
  if (metadata.packages.some((p) => p.name === HOOK)) {
    const treeArgs = [
      'tree',
      '-p',
      HOOK,
      '-e',
      'normal,build',
      '--target',
      'all',
      '--prefix',
      'none',
    ];
    if (manifestFlag !== -1) treeArgs.push('--manifest-path', args[manifestFlag + 1] ?? '');
    violations.push(...checkHookTree(execFileSync('cargo', treeArgs, { encoding: 'utf8' })));
  }
  if (violations.length > 0) {
    console.error('crate dependency check FAILED:');
    for (const v of violations) console.error(`  - ${v}`);
    process.exit(1);
  }
  console.log(`crate dependency check passed (${metadata.packages.length} workspace members)`);
}

if (import.meta.main) main(process.argv.slice(2));
