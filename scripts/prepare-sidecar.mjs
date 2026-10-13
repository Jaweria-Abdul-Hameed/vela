// Builds `vela-hook` in release mode and places it where the Tauri bundler expects a sidecar: `binaries/vela-hook-<target triple>`.
// The installed file is `vela-hook.exe` next to the application, which is where the profile generator looks for it.
import { execFileSync } from 'node:child_process';
import { copyFileSync, mkdirSync } from 'node:fs';
import { join } from 'node:path';

const root = new URL('..', import.meta.url).pathname.replace(/^\/([A-Za-z]:)/, '$1');
const triple = process.env.TAURI_TARGET_TRIPLE ?? 'x86_64-pc-windows-msvc';
execFileSync('cargo', ['build', '--release', '--locked', '-p', 'vela-hook', '--target', triple], { cwd: root, stdio: 'inherit' });
const outDir = join(root, 'apps', 'desktop', 'src-tauri', 'binaries');
mkdirSync(outDir, { recursive: true });
const exe = triple.includes('windows') ? '.exe' : '';
const dest = join(outDir, `vela-hook-${triple}${exe}`);
copyFileSync(join(root, 'target', triple, 'release', `vela-hook${exe}`), dest);
console.log(`sidecar ready: ${dest}`);
