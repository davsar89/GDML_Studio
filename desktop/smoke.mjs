// End-to-end check of the desktop shell on the real path: the app starts its
// backend, opens a sample, saves through the native-dialog bridge, asks before
// closing over unsaved changes, and takes the backend down with it.
//   npm run smoke                                  (repo build, via `electron .`)
//   npm run smoke -- "dist\win-unpacked\GDML Studio.exe"
import { _electron as electron } from 'playwright-core';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';

const exe = process.argv[2];
const sample = path.resolve(import.meta.dirname, '../sample_data/solids.gdml');
const out = fs.mkdtempSync(path.join(os.tmpdir(), 'gdml-smoke-'));
const app = await electron.launch(exe ? { executablePath: exe } : { args: ['.'], cwd: import.meta.dirname });
const page = await app.firstWindow();
const backendUrl = new URL(page.url()).origin;
assert.equal(await page.title(), 'GDML Studio');

const [chooser] = await Promise.all([page.waitForEvent('filechooser'), page.getByRole('button', { name: 'Open File(s)' }).click()]);
await chooser.setFiles(sample);
await page.locator('.document-name', { hasText: 'solids.gdml' }).waitFor();
await page.locator('canvas').waitFor();
await page.waitForTimeout(1500);
await page.screenshot({ path: path.join(out, 'window.png') });

// Save: the main process's dialog answers with a path, and the file must land there.
const saved = path.join(out, 'saved.gdml');
await app.evaluate(({ dialog }, filePath) => { dialog.showSaveDialog = async () => ({ canceled: false, filePath }); }, saved);
await page.getByRole('button', { name: 'Save', exact: true }).click();
await page.waitForFunction(() => document.querySelector('.document-name')?.textContent?.includes('saved.gdml'));
assert.match(fs.readFileSync(saved, 'utf8'), /<gdml[\s>]/);

// Unsaved changes: the page objects to unloading, as Toolbar.tsx does when dirty.
await page.evaluate(() => addEventListener('beforeunload', (e) => e.preventDefault()));
const closeAnswering = (choice) => app.evaluate(({ dialog, BrowserWindow }, choice) => {
  dialog.showMessageBoxSync = () => choice;
  BrowserWindow.getAllWindows()[0].close();
}, choice);
await closeAnswering(1); // Cancel
await page.waitForTimeout(1000);
assert.equal(page.isClosed(), false, 'Cancel must keep the window open');
const exited = new Promise((resolve, reject) => {
  app.process().once('exit', resolve);
  setTimeout(() => reject(new Error('Leave must close the app')), 10000).unref();
});
await closeAnswering(0); // Leave
await exited;

// The backend must be gone with the app.
let backendAlive = true;
for (let i = 0; i < 20 && backendAlive; i++) {
  backendAlive = await fetch(backendUrl).then(() => true, () => false);
  if (backendAlive) await new Promise((resolve) => setTimeout(resolve, 100));
}
assert.equal(backendAlive, false, `backend still answering on ${backendUrl}`);
console.log(`smoke OK (${backendUrl}); screenshot: ${path.join(out, 'window.png')}`);
