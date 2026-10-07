// Build: tsc -> dist/js, concatenated CSS -> dist/app.css, static files copied. No bundler:
// the browser loads ES modules directly (HTTP/2 or the local app scheme makes that cheap).
import { execFileSync } from 'node:child_process';
import { cpSync, mkdirSync, readFileSync, readdirSync, rmSync, writeFileSync } from 'node:fs';

rmSync('dist', { recursive: true, force: true });
mkdirSync('dist', { recursive: true });
execFileSync('node_modules/.bin/tsc', ['-p', 'tsconfig.build.json'], { stdio: 'inherit' });
const css = readdirSync('styles').filter((f) => f.endsWith('.css')).sort()
  .map((f) => readFileSync(`styles/${f}`, 'utf8')).join('\n');
writeFileSync('dist/app.css', css);
cpSync('index.html', 'dist/index.html');
cpSync('public', 'dist', { recursive: true });
console.log('built dist/');
