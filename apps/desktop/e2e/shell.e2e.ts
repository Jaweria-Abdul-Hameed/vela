import { expect, test } from '@playwright/test';

test('the renderer starts in a plain browser and reports that no core is attached', async ({
  page,
}) => {
  await page.goto('/');
  await expect(page.getByRole('heading', { name: 'Vela' })).toBeVisible();
  await expect(page.getByRole('alert')).toContainText('Core unavailable');
});

test('software GL provides a WebGL context', async ({ page }) => {
  await page.goto('/');
  const hasWebGl = await page.evaluate(() => {
    const canvas = document.createElement('canvas');
    return canvas.getContext('webgl2') !== null || canvas.getContext('webgl') !== null;
  });
  expect(hasWebGl).toBe(true);
});
