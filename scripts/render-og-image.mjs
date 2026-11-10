// Renders scripts/og-image.html to public/og-image.png (1200×630) with headless Chrome.
// Usage: npm run og-image   (set CHROME to the Chrome/Chromium binary if it isn't in the macOS default place)
import {execFileSync} from 'node:child_process';
import {resolve} from 'node:path';
import {pathToFileURL} from 'node:url';

const chrome = process.env.CHROME || '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome';
execFileSync(chrome, [
  '--headless=new', '--disable-gpu', '--hide-scrollbars', '--force-device-scale-factor=1',
  '--window-size=1200,630', '--virtual-time-budget=5000',
  `--screenshot=${resolve('public/og-image.png')}`, pathToFileURL(resolve('scripts/og-image.html')).href,
], {stdio: 'inherit'});
