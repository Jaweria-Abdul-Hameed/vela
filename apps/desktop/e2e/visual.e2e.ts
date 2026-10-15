// U18: visual regression. Each scene is the real renderer over a fixed fixture (src/visual/fixtures.ts) with motion off. Baselines
// live in visual.e2e.ts-snapshots and are produced on the CI platform; `npm run test:e2e -- --update-snapshots` (or the
// "Update visual baselines" workflow) regenerates them deliberately, and a reviewer looks at the diff before it is committed.
import { expect, test, type Page } from '@playwright/test';

test.use({ viewport: { width: 1280, height: 800 }, deviceScaleFactor: 1, colorScheme: 'dark' });

async function plan(page: Page) {
  await page.getByRole('button', { name: 'demo-app', exact: true }).click();
  await page.getByRole('button', { name: 'Plan a build' }).click();
}

const scenes: Record<string, (page: Page) => Promise<void>> = {
  home: async (page) => {
    await expect(page.getByRole('heading', { name: 'Projects' })).toBeVisible();
  },
  'active-graph': async (page) => {
    await plan(page);
    await expect(page.getByRole('region', { name: 'Run', exact: true })).toBeVisible();
  },
  'worker-detail': async (page) => {
    await plan(page);
    await expect(page.getByRole('region', { name: 'Run', exact: true })).toBeVisible();
    await page.getByRole('button', { name: /^T-3,/ }).click();
    await expect(page.getByRole('complementary', { name: 'Ticket T-3' })).toBeVisible();
  },
  'conflict-warning': async (page) => {
    await plan(page);
    await page.getByRole('button', { name: 'T-2 and T-3' }).click();
    await expect(page.getByText(/db\/schema\.sql/)).toBeVisible();
  },
  'needs-human': async (page) => {
    await plan(page);
    await expect(page.getByText(/both public and private/)).toBeVisible();
  },
  completed: async (page) => {
    await plan(page);
    await expect(page.getByText(/pull request/i).first()).toBeVisible();
  },
  'reduced-motion': async (page) => {
    await page.emulateMedia({ reducedMotion: 'reduce' });
    await plan(page);
    await expect(page.getByRole('region', { name: 'Run', exact: true })).toBeVisible();
  },
};

for (const [state, steps] of Object.entries(scenes)) {
  test(`scene: ${state}`, async ({ page }) => {
    await page.goto(`/visual.html?state=${state}`);
    await steps(page);
    // Let the canvases draw their still frame.
    await page.waitForTimeout(1200);
    await expect(page).toHaveScreenshot(`${state}.png`, {
      animations: 'disabled',
      maxDiffPixelRatio: 0.015,
    });
  });
}
