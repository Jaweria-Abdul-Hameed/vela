// Module-skeleton check (F01; SHARED_SURFACE_PROTOCOL.md section 1).
//
// Verifies that every crate and package still has exactly the module skeleton recorded in
// `scripts/module-skeleton.json`: a missing module or an extra module in a closed declaration list fails.
// Tickets write inside their own module and never edit another module's declaration list.

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

/** @typedef {null | { closed: boolean, modules: Record<string, SkeletonNode> }} SkeletonNode */
/** @typedef {{ src: string, entry: string, closed?: boolean, modules: Record<string, SkeletonNode> }} CrateSkeleton */
/** @typedef {{ rust: Record<string, CrateSkeleton>, typescript: Record<string, string[]> }} Skeleton */

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(scriptDir, '..');

// Removes line and block comments in a single pass, so a block-comment opener inside a line comment is inert.
function stripComments(/** @type {string} */ source) {
  let out = '';
  let i = 0;
  while (i < source.length) {
    const two = source.slice(i, i + 2);
    if (two === '//') {
      while (i < source.length && source[i] !== '\n') i++;
    } else if (two === '/*') {
      const end = source.indexOf('*/', i + 2);
      i = end === -1 ? source.length : end + 2;
    } else {
      out += source[i];
      i++;
    }
  }
  return out;
}

/**
 * Names declared by top-level `mod x;` / `mod x {` items, ignoring comments, `#[cfg(test)]` modules (also behind
 * stacked or same-line attributes), and anything nested inside another item's braces.
 */
export function declaredModules(/** @type {string} */ source) {
  const lines = stripComments(source).split('\n');
  /** @type {string[]} */
  const names = [];
  const decl =
    /^\s*((?:#\[[^\]]*\]\s*)*)(?:pub(?:\([^)]*\))?\s+)?mod\s+([A-Za-z_][A-Za-z0-9_]*)\s*[;{]/;
  let depth = 0;
  for (const [i, line] of lines.entries()) {
    const match = depth === 0 ? decl.exec(line) : null;
    if (match?.[2]) {
      let attrs = match[1] ?? '';
      for (let prev = i - 1; prev >= 0 && /^\s*(#\[|$)/.test(lines[prev] ?? ''); prev--) {
        attrs += lines[prev];
      }
      if (!/#\[cfg\(test\)\]/.test(attrs)) names.push(match[2]);
    }
    for (const ch of line) {
      if (ch === '{') depth++;
      else if (ch === '}') depth--;
    }
  }
  return names;
}

/** `dir/name/mod.rs` or `dir/name.rs`, whichever exists. */
function moduleFile(/** @type {string} */ dir, /** @type {string} */ name) {
  for (const candidate of [path.join(dir, name, 'mod.rs'), path.join(dir, `${name}.rs`)]) {
    if (fs.existsSync(candidate)) return candidate;
  }
  return null;
}

/**
 * @param {string} root repository root that messages are relative to
 * @param {string} label crate name used in messages
 * @param {string} file the declaring file (lib.rs, main.rs or a module file)
 * @param {string} dir the directory that holds this module's children
 * @param {string} modPath dotted module path used in messages
 * @param {Record<string, SkeletonNode>} expected
 * @param {boolean} closed
 * @param {string[]} violations
 */
function checkModules(root, label, file, dir, modPath, expected, closed, violations) {
  const declared = new Set(declaredModules(fs.readFileSync(file, 'utf8')));
  const where = path.relative(root, file).replaceAll('\\', '/');
  for (const name of Object.keys(expected)) {
    if (!declared.has(name)) {
      violations.push(`${label}: module ${modPath}${name} is missing from ${where}`);
    }
  }
  if (closed) {
    for (const name of declared) {
      if (!Object.hasOwn(expected, name)) {
        violations.push(
          `${label}: extra module ${modPath}${name} declared in ${where}; it is not in the skeleton (raise a follow-up instead)`,
        );
      }
    }
  }
  for (const [name, node] of Object.entries(expected)) {
    if (!declared.has(name)) continue;
    const child = moduleFile(dir, name);
    if (!child) {
      violations.push(
        `${label}: module file for ${modPath}${name} not found under ${path.relative(root, dir).replaceAll('\\', '/')}`,
      );
      continue;
    }
    if (node) {
      checkModules(
        root,
        label,
        child,
        path.join(dir, name),
        `${modPath}${name}::`,
        node.modules,
        node.closed,
        violations,
      );
    }
  }
}

/**
 * @param {Skeleton} skeleton
 * @param {string} [root] repository root (defaults to this repository)
 * @returns {string[]} violations; empty when every skeleton is intact
 */
export function checkModuleSkeleton(skeleton, root = repoRoot) {
  /** @type {string[]} */
  const violations = [];
  const abs = (/** @type {string} */ p) => path.resolve(root, p);

  for (const [crate, c] of Object.entries(skeleton.rust)) {
    const src = abs(c.src);
    const entry = path.join(src, c.entry);
    if (!fs.existsSync(entry)) {
      violations.push(
        `${crate}: ${path.relative(root, entry).replaceAll('\\', '/')} does not exist`,
      );
      continue;
    }
    checkModules(root, crate, entry, src, '', c.modules, c.closed !== false, violations);
  }

  for (const [pkg, files] of Object.entries(skeleton.typescript)) {
    for (const file of files) {
      const full = abs(path.join(pkg, file));
      const stem = full.replace(/\.tsx?$/, '');
      if (!fs.existsSync(`${stem}.ts`) && !fs.existsSync(`${stem}.tsx`)) {
        violations.push(`${pkg}: module ${file} is missing`);
      }
    }
  }
  return violations;
}

function main() {
  /** @type {Skeleton} */
  const skeleton = JSON.parse(
    fs.readFileSync(path.join(scriptDir, 'module-skeleton.json'), 'utf8'),
  );
  const violations = checkModuleSkeleton(skeleton);
  if (violations.length > 0) {
    console.error('module skeleton check FAILED:');
    for (const v of violations) console.error(`  - ${v}`);
    process.exit(1);
  }
  const rust = Object.keys(skeleton.rust).length;
  const ts = Object.keys(skeleton.typescript).length;
  console.log(`module skeleton check passed (${rust} Rust crates, ${ts} TypeScript packages)`);
}

if (import.meta.main) main();
