// Secret scan of a pushed range (X05). Looks at file names and at added lines; rules are data in security/secret-rules.json
// and exceptions live in security/audit-allowlist.json (scope "secrets", id = rule id, optional "path" regex).
// Usage: node scripts/scan-secrets.mjs <base>..<head>   (or a single revision to scan one commit's changes)
import { execFileSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { validateAllowlist } from './check-audit-allowlist.mjs';

const root = path.join(path.dirname(fileURLToPath(import.meta.url)), '..');

/** Builds a JS RegExp from a rule pattern; a leading `(?i)` means case-insensitive. */
export function compile(pattern) {
  const insensitive = pattern.startsWith('(?i)');
  return new RegExp(insensitive ? pattern.slice(4) : pattern, insensitive ? 'i' : '');
}

export function loadRules(rulesPath = path.join(root, 'security', 'secret-rules.json')) {
  const raw = JSON.parse(fs.readFileSync(rulesPath, 'utf8'));
  return {
    fileNames: raw.fileNames.map((r) => ({
      ...r,
      re: compile(r.pattern),
      exceptions: (r.exceptions ?? []).map(compile),
    })),
    content: raw.content.map((r) => ({ ...r, re: compile(r.pattern) })),
  };
}

/** Findings for one file: its name, and each added line. */
export function scanFile(rules, file, addedLines) {
  const findings = [];
  const normalized = file.replaceAll('\\', '/');
  for (const rule of rules.fileNames) {
    if (rule.re.test(normalized) && !rule.exceptions.some((e) => e.test(normalized))) {
      findings.push({ rule: rule.id, file: normalized, line: 0 });
    }
  }
  for (const { line, text } of addedLines) {
    for (const rule of rules.content) {
      if (rule.re.test(text)) findings.push({ rule: rule.id, file: normalized, line });
    }
  }
  return findings;
}

/** Parses `git diff -U0` output into `{ file, lines: [{ line, text }] }` records (added lines only). */
export function parseDiff(diff) {
  const files = [];
  let current = null;
  let next = 0;
  for (const raw of diff.split('\n')) {
    if (raw.startsWith('+++ ')) {
      const name = raw.slice(4).replace(/^b\//, '');
      current = name === '/dev/null' ? null : { file: name, lines: [] };
      if (current) files.push(current);
    } else if (raw.startsWith('@@')) {
      const m = /\+(\d+)/.exec(raw);
      next = m ? Number(m[1]) : 0;
    } else if (current && raw.startsWith('+') && !raw.startsWith('+++')) {
      current.lines.push({ line: next, text: raw.slice(1) });
      next += 1;
    }
  }
  return files;
}

export function applyAllowlist(findings, allowlist, today = new Date()) {
  const problems = validateAllowlist(allowlist, today);
  if (problems.length > 0) throw new Error(`invalid allowlist: ${problems.join('; ')}`);
  const entries = allowlist.entries.filter((e) => e.scope === 'secrets');
  return findings.filter(
    (f) =>
      !entries.some(
        (e) => e.id === f.rule && (e.path === undefined || new RegExp(e.path).test(f.file)),
      ),
  );
}

function git(args, cwd) {
  return execFileSync('git', args, { cwd, encoding: 'utf8', maxBuffer: 256 * 1024 * 1024 });
}

export function scanRange(range, cwd = root, rules = loadRules(), allowlist = undefined) {
  const names = git(['diff', '--name-only', '--diff-filter=AM', range], cwd)
    .split('\n')
    .filter(Boolean);
  const diff = git(['diff', '-U0', '--no-color', '--diff-filter=AM', range], cwd);
  const byFile = new Map(parseDiff(diff).map((f) => [f.file, f.lines]));
  const findings = names.flatMap((name) => scanFile(rules, name, byFile.get(name) ?? []));
  const list =
    allowlist ??
    JSON.parse(fs.readFileSync(path.join(root, 'security', 'audit-allowlist.json'), 'utf8'));
  return applyAllowlist(findings, list);
}

if (
  process.argv[1] &&
  path.resolve(process.argv[1]) === path.resolve(fileURLToPath(import.meta.url))
) {
  const range = process.argv[2];
  if (!range) {
    console.error('usage: node scripts/scan-secrets.mjs <base>..<head>');
    process.exit(2);
  }
  const findings = scanRange(range);
  if (findings.length > 0) {
    for (const f of findings)
      console.error(`secret scan: ${f.rule} in ${f.file}${f.line ? `:${f.line}` : ''}`);
    process.exit(1);
  }
  console.log(`secret scan clean for ${range}`);
}
