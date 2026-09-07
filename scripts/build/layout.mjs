import { join } from 'node:path';

export const buildLayout = Object.freeze({
  root: 'build',
  artifacts: join('build', 'artifacts'),
  cargoCache: join('build', 'cache', 'cargo'),
  localCargoCache: join('build', 'cache', 'cargo', 'local'),
  viteCache: join('build', 'cache', 'vite'),
  webStaging: join('build', 'staging', 'web'),
  testResults: join('build', 'test', 'results'),
  testScreenshots: join('build', 'test', 'screenshots'),
  preview: join('build', 'preview'),
  previewSettings: join('build', 'preview', 'settings.json'),
  lock: join('build', 'task.lock'),
});

export function inBuild(root, path) {
  return join(root, path);
}
