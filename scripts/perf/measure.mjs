/* global window */
// PF1: performance harness. Captures the per-run environment (read-only queries only), measures the pure graph and event-burst code in
// Node, and measures the real renderer in headless Chromium (software GL) per graphics mode. Writes a report that says plainly whether
// it is a reference run or a non-reference exploratory run (docs/ui/PERFORMANCE_BUDGET.md, "Conditions for a reference run").
//
// Usage: node scripts/perf/measure.mjs [--out docs/ui/PERFORMANCE_RESULTS_<date>.md]
import { execFileSync, spawn } from 'node:child_process';
import { mkdtempSync, writeFileSync } from 'node:fs';
import { tmpdir, cpus, totalmem, release, platform } from 'node:os';
import { join, resolve } from 'node:path';
import { pathToFileURL } from 'node:url';

const root = resolve(import.meta.dirname, '..', '..');
const outArg = process.argv.indexOf('--out');
const stamp = new Date().toISOString().slice(0, 10);
const outFile = resolve(
  root,
  outArg > 0 ? process.argv[outArg + 1] : `docs/ui/PERFORMANCE_RESULTS_${stamp}.md`,
);

function ps(command) {
  try {
    return execFileSync('powershell', ['-NoProfile', '-NonInteractive', '-Command', command], {
      encoding: 'utf8',
      timeout: 20_000,
    }).trim();
  } catch {
    return 'not determined';
  }
}

function git(...args) {
  try {
    return execFileSync('git', args, { cwd: root, encoding: 'utf8' }).trim();
  } catch {
    return 'unknown';
  }
}

/** Per-run environment capture. Read-only: nothing is installed or changed. */
function captureEnvironment() {
  const win = platform() === 'win32';
  const env = {
    profile: 'REF-HW-1 (claimed; hardware match not verified by this script)',
    when: new Date().toISOString(),
    commit: git('rev-parse', 'HEAD'),
    node: process.version,
    os: `${platform()} ${release()}`,
    cpu: `${cpus()[0]?.model ?? 'unknown'} (${cpus().length} logical)`,
    ramGiB: (totalmem() / 2 ** 30).toFixed(2),
  };
  if (win) {
    env.powerScheme = ps('(powercfg /getactivescheme)');
    env.battery = ps(
      "$b = Get-CimInstance Win32_Battery -ErrorAction SilentlyContinue; if ($b) { 'status=' + $b.BatteryStatus + ' charge=' + $b.EstimatedChargeRemaining + '%' } else { 'no battery reported' }",
    );
    env.gpus = ps(
      "(Get-CimInstance Win32_VideoController | ForEach-Object { $_.Name + ' driver ' + $_.DriverVersion }) -join '; '",
    );
    env.display = ps(
      "(Get-CimInstance Win32_VideoController | ForEach-Object { '' + $_.CurrentHorizontalResolution + 'x' + $_.CurrentVerticalResolution + '@' + $_.CurrentRefreshRate + 'Hz' }) -join '; '",
    );
    env.webview2 = ps(
      "(Get-ItemProperty 'HKLM:\\SOFTWARE\\WOW6432Node\\Microsoft\\EdgeUpdate\\Clients\\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}' -ErrorAction SilentlyContinue).pv",
    );
  }
  env.acPower = /status=2\b|no battery/.test(env.battery ?? '')
    ? 'AC (or no battery)'
    : 'battery or not determined';
  env.acerMode = 'not determined';
  env.exclusive = 'NOT guaranteed: this run is not isolated from other work on the machine';
  return env;
}

/** Bundles the pure perf module with rolldown and imports it. */
async function loadPerf() {
  const dir = mkdtempSync(join(tmpdir(), 'vela-perf-'));
  const out = join(dir, 'perf.mjs');
  const rolldown = resolve(
    root,
    'node_modules',
    '.bin',
    process.platform === 'win32' ? 'rolldown.cmd' : 'rolldown',
  );
  execFileSync(
    rolldown,
    ['packages/ui/src/canvas/graph/perf.ts', '--format', 'esm', '--file', out],
    { cwd: root, stdio: 'ignore', shell: process.platform === 'win32' },
  );
  return import(pathToFileURL(out).href);
}

function stats(samples) {
  const s = [...samples].sort((a, b) => a - b);
  const pick = (p) => s[Math.min(s.length - 1, Math.floor(s.length * p))];
  return { n: s.length, median: pick(0.5), p95: pick(0.95), max: s[s.length - 1] };
}

function fmt(n, d = 3) {
  return Number(n).toFixed(d);
}

async function measureCompute() {
  const perf = await loadPerf();
  const vp = { width: 1707, height: 1067 };
  const rows = [];
  for (const nodes of [100, 500]) {
    const { nodes: ns } = perf.fixtureGraph(nodes);
    for (const zoom of [0.25, 0.5, 1]) {
      const view = { x: 400, y: 400, zoom };
      const times = [];
      for (let i = 0; i < 400; i++) {
        const t0 = performance.now();
        perf.labelled(perf.cull(ns, view, vp), view, 'T-1', perf.labelBudget(zoom));
        times.push(performance.now() - t0);
      }
      rows.push({ nodes, zoom, ...stats(times.slice(50)) });
    }
  }
  // Event burst: 1000 events in one second's worth of pushes, folded per frame.
  let longest = 0;
  const flushed = [];
  const queue = [];
  const batcher = new perf.Batcher(
    (b) => flushed.push(b.length),
    (run) => queue.push(run),
  );
  const t0 = performance.now();
  for (let frame = 0; frame < 60; frame++) {
    const f0 = performance.now();
    for (let i = 0; i < 17; i++) batcher.push(`T-${(frame * 17 + i) % 500}`, i);
    queue.shift()?.();
    longest = Math.max(longest, performance.now() - f0);
  }
  return {
    rows,
    burst: {
      events: 60 * 17,
      frames: 60,
      longestFrameMs: longest,
      totalMs: performance.now() - t0,
      flushed: flushed.length,
    },
  };
}

async function waitFor(url, ms = 30_000) {
  const end = Date.now() + ms;
  while (Date.now() < end) {
    try {
      const r = await fetch(url);
      if (r.ok) return;
    } catch {
      /* not up yet */
    }
    await new Promise((r) => setTimeout(r, 300));
  }
  throw new Error(`${url} did not start`);
}

async function measureBrowser() {
  let chromium;
  try {
    ({ chromium } = await import(
      pathToFileURL(resolve(root, 'node_modules', 'playwright', 'index.mjs')).href
    ));
  } catch {
    return { skipped: 'Playwright is not installed' };
  }
  const port = 1431;
  const server = spawn(
    process.platform === 'win32' ? 'npx.cmd' : 'npx',
    ['vite', '--port', String(port), '--strictPort'],
    { cwd: resolve(root, 'apps', 'desktop'), stdio: 'ignore', shell: process.platform === 'win32' },
  );
  const results = [];
  try {
    await waitFor(`http://localhost:${port}/`);
    const browser = await chromium.launch({
      args: [
        '--use-gl=angle',
        '--use-angle=swiftshader',
        '--enable-unsafe-swiftshader',
        '--ignore-gpu-blocklist',
      ],
    });
    for (const mode of ['efficiency', 'balanced', 'full']) {
      for (const reduced of [false, true]) {
        const context = await browser.newContext({
          viewport: { width: 1707, height: 1067 },
          deviceScaleFactor: 1.5,
          reducedMotion: reduced ? 'reduce' : 'no-preference',
        });
        const page = await context.newPage();
        await page.addInitScript(() => {
          window.__raf = 0;
          const orig = window.requestAnimationFrame.bind(window);
          window.requestAnimationFrame = (cb) => {
            window.__raf += 1;
            return orig(cb);
          };
          window.__long = [];
          new PerformanceObserver((l) =>
            l.getEntries().forEach((e) => window.__long.push(e.duration)),
          ).observe({
            type: 'longtask',
            buffered: true,
          });
        });
        const t0 = Date.now();
        await page.goto(`http://localhost:${port}/`);
        await page.waitForSelector('#root *', { timeout: 30_000 });
        const coldMs = Date.now() - t0;
        const select = page.getByRole('combobox', { name: 'Graphics' });
        if (await select.count()) await select.selectOption(mode);
        // Let the mode's idle suspension elapse (no input): efficiency 2 s, balanced 8 s, full 20 s, plus a margin.
        await page.waitForTimeout({ efficiency: 4000, balanced: 10_000, full: 22_000 }[mode]);
        const before = await page.evaluate(() => window.__raf);
        await page.waitForTimeout(5000);
        const after = await page.evaluate(() => window.__raf);
        const long = await page.evaluate(() => window.__long);
        const heap = await page.evaluate(() =>
          performance.memory ? performance.memory.usedJSHeapSize : null,
        );
        results.push({
          mode,
          reducedMotion: reduced,
          coldLaunchMs: coldMs,
          rafPerSecondIdle: (after - before) / 5,
          longTasks: long.length,
          longestTaskMs: long.length ? Math.max(...long) : 0,
          heapMiB: heap ? heap / 2 ** 20 : null,
        });
        await context.close();
      }
    }
    await browser.close();
  } finally {
    server.kill();
    if (process.platform === 'win32') {
      try {
        execFileSync('taskkill', ['/PID', String(server.pid), '/T', '/F'], { stdio: 'ignore' });
      } catch {
        /* already gone */
      }
    }
  }
  return { results };
}

const environment = captureEnvironment();
const compute = await measureCompute();
const browser = await measureBrowser();

const lines = [];
lines.push(`# Performance results ${stamp} (NON-REFERENCE exploratory run)`);
lines.push('');
lines.push(
  '**This is a non-reference exploratory run.** It is not release evidence and satisfies no budget: the machine was not exclusive (builds and other work were running), rendering used software GL in headless Chromium rather than the GPU and WebView2, and the reference-hardware conditions in `PERFORMANCE_BUDGET.md` were not all verified. Budgets are unchanged.',
);
lines.push('');
lines.push('## Environment capture');
lines.push('');
for (const [k, v] of Object.entries(environment)) lines.push(`- **${k}**: ${v}`);
lines.push('');
lines.push('## Graph code (Node, pure functions, per frame)');
lines.push('');
lines.push('| Nodes | Zoom | median ms | p95 ms | max ms |');
lines.push('|---|---|---|---|---|');
for (const r of compute.rows)
  lines.push(`| ${r.nodes} | ${r.zoom} | ${fmt(r.median)} | ${fmt(r.p95)} | ${fmt(r.max)} |`);
lines.push('');
lines.push(
  `Event burst: ${compute.burst.events} events folded over ${compute.burst.frames} frames into ${compute.burst.flushed} flushes; longest frame ${fmt(compute.burst.longestFrameMs)} ms (budget: no task over 100 ms).`,
);
lines.push('');
lines.push('## Renderer (headless Chromium, software GL, DPR 1.5)');
lines.push('');
if (browser.skipped) {
  lines.push(`Not measured: ${browser.skipped}.`);
} else {
  lines.push(
    '| Mode | Reduced motion | Cold launch ms | rAF/s idle (budget 0) | Long tasks | Longest task ms | JS heap MiB |',
  );
  lines.push('|---|---|---|---|---|---|---|');
  for (const r of browser.results) {
    lines.push(
      `| ${r.mode} | ${r.reducedMotion} | ${r.coldLaunchMs} | ${fmt(r.rafPerSecondIdle, 1)} | ${r.longTasks} | ${fmt(r.longestTaskMs, 0)} | ${r.heapMiB === null ? 'n/a' : fmt(r.heapMiB, 1)} |`,
    );
  }
}
lines.push('');
lines.push('## Not measured by this harness');
lines.push('');
lines.push(
  '- Frame rate of the 100- and 500-node WebGL graph on the real GPU through WebView2 (needs the built desktop app and a reference run).',
);
lines.push(
  '- Minimized and hidden behaviour, unfocused throttling, GPU frame submissions, UI-process CPU over 60 s, memory across 100 canvas mount/unmount cycles on the GPU.',
);
lines.push(
  '- AT-009 (orchestration events still journaled while minimized): the journal is written by the core, independent of any window; confirming it with the real minimized window needs the desktop app.',
);
lines.push('');
writeFileSync(outFile, lines.join('\n') + '\n');
console.log(`wrote ${outFile}`);
