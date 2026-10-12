// Validates security/audit-allowlist.json (X05): every exception needs a justification and a future expiry.
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

export const SCOPES = ['secrets', 'rust-advisory', 'npm-advisory', 'license'];

/**
 * @param {unknown} document the parsed allowlist file
 * @param {Date} today
 * @returns {string[]} one message per problem; empty means valid
 */
export function validateAllowlist(document, today = new Date()) {
  const problems = [];
  if (typeof document !== 'object' || document === null || !Array.isArray(document.entries)) {
    return ['the allowlist must be an object with an "entries" array'];
  }
  document.entries.forEach((entry, i) => {
    const where = `entries[${i}]${entry && entry.id ? ` (${entry.id})` : ''}`;
    if (typeof entry !== 'object' || entry === null) {
      problems.push(`${where}: must be an object`);
      return;
    }
    if (typeof entry.id !== 'string' || entry.id.trim() === '')
      problems.push(`${where}: missing "id"`);
    if (!SCOPES.includes(entry.scope))
      problems.push(`${where}: "scope" must be one of ${SCOPES.join(', ')}`);
    if (typeof entry.justification !== 'string' || entry.justification.trim().length < 10) {
      problems.push(`${where}: a justification of at least 10 characters is required`);
    }
    if (typeof entry.expires !== 'string' || !/^\d{4}-\d{2}-\d{2}$/.test(entry.expires)) {
      problems.push(`${where}: "expires" must be a date written YYYY-MM-DD`);
    } else {
      const expiry = new Date(`${entry.expires}T23:59:59Z`);
      if (Number.isNaN(expiry.getTime())) problems.push(`${where}: "expires" is not a real date`);
      else if (expiry < today) problems.push(`${where}: expired on ${entry.expires}`);
    }
  });
  return problems;
}

const here = path.dirname(fileURLToPath(import.meta.url));
if (
  process.argv[1] &&
  path.resolve(process.argv[1]) === path.resolve(fileURLToPath(import.meta.url))
) {
  const file = path.join(here, '..', 'security', 'audit-allowlist.json');
  const problems = validateAllowlist(JSON.parse(fs.readFileSync(file, 'utf8')));
  if (problems.length > 0) {
    console.error(problems.join('\n'));
    process.exit(1);
  }
  console.log('audit allowlist ok');
}
