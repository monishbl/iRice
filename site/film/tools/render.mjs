#!/usr/bin/env node
/**
 * render.mjs: render a film to video, frame-exact.
 * (iRice's copy of the product-film kit's tool, MIT: adds software WebGL for
 * the glass layer, and PLAYWRIGHT_FROM to find Playwright.)
 *
 * A film exposes window.__film = { DUR, seek(t), pause() } and draws each
 * frame as a pure function of t. Instead of screen-recording (which drops
 * frames whenever the machine is busy) this seeks to every frame time,
 * screenshots it and pipes the frames to ffmpeg. The output never stutters,
 * however slow the machine.
 *
 *   node tools/render.mjs <film.html> [options]
 *
 *   --root <dir>      folder to serve (default: cwd); the film path is relative to it
 *   --fps <n>         frames per second (default 30; 60 for UI-heavy films with fast moves)
 *   --scale <n>       device pixel ratio: 2 renders 1920×1080 CSS px as 3840×2160
 *   --shutter <n>     motion blur: average n sub-frames across the half frame after each frame time
 *                     (180° shutter). 4–8 is plenty; put hard cuts on the frame grid (t = k / fps)
 *   --from <s> --to <s>   render a range (for review passes)
 *   --out <file.mp4>  default: next to the film, <name>.mp4
 *   --audio <file>    mux this soundtrack (wav/mp3/m4a). A film that defines
 *                     __film.audioWav() (base64 WAV, rendered offline) is muxed automatically
 *   --webm            also write a VP9 .webm (VS Code's preview and some Linux browsers lack H.264)
 *   --stills <dir>    also write every frame as a JPEG, for review
 *   --png             lossless PNG screenshots (slower; for flat-colour films where JPEG rings)
 *
 * Needs ffmpeg on PATH and Playwright (npm i -D playwright; npx playwright install chromium).
 * CHROME=/path/to/chrome uses a specific browser.
 *
 * Colour: screenshots are full-range RGB, and H.264 players assume limited
 * range, so frames are converted with scale=in_range=full:out_range=tv before
 * yuv420p. Without that, blacks lift and whites dull on every player.
 */
import { spawn } from 'node:child_process';
import { mkdirSync, rmSync, writeFileSync } from 'node:fs';
import { basename, dirname, join, relative, resolve } from 'node:path';
import { serve } from './serve.mjs';

const argv = process.argv.slice(2);
const opt = (k, d) => {
	const i = argv.indexOf(`--${k}`);
	if (i < 0) return d;
	const v = argv[i + 1];
	argv.splice(i, 2);
	return v;
};
const flag = (k) => {
	const i = argv.indexOf(`--${k}`);
	return i >= 0 ? (argv.splice(i, 1), true) : false;
};
const ROOT = resolve(opt('root', '.'));
const FPS = Number(opt('fps', 30));
const SCALE = Number(opt('scale', 1));
const SHUTTER = Math.max(1, Number(opt('shutter', 1)));
const FROM = Number(opt('from', 0));
const TO = opt('to');
const AUDIO = opt('audio');
const STILLS = opt('stills');
let OUT = opt('out');
const WEBM = flag('webm');
const PNG = flag('png');
const film = argv[0];
if (!film) {
	console.error('usage: node tools/render.mjs <film.html> [--fps 30] [--scale 2] [--shutter 6] [--webm] …');
	process.exit(1);
}
const filmAbs = resolve(ROOT, film);
OUT = resolve(OUT || join(dirname(filmAbs), basename(filmAbs, '.html') + '.mp4'));
const tmp = OUT.replace(/\.mp4$/, '');

let chromium;
/* iRice: PLAYWRIGHT_FROM names a package.json whose node_modules has Playwright
   (the product-film kit's), since this copy lives outside it */
if (process.env.PLAYWRIGHT_FROM) {
	({ chromium } = (await import('node:module')).createRequire(process.env.PLAYWRIGHT_FROM)('playwright'));
} else {
	try {
		({ chromium } = await import('playwright'));
	} catch {
		({ chromium } = await import('@playwright/test'));
	}
}

const run = (args) => new Promise((ok, fail) => spawn('ffmpeg', ['-y', '-loglevel', 'error', ...args], { stdio: 'inherit' }).on('close', (c) => (c ? fail(new Error('ffmpeg exited ' + c)) : ok())));

const server = await serve(ROOT);
const browser = await chromium.launch({ executablePath: process.env.CHROME || undefined, args: ['--disable-gpu-vsync', '--force-color-profile=srgb', /* iRice: WebGL without a GPU (the glass) */ '--use-angle=swiftshader', '--enable-unsafe-swiftshader', '--ignore-gpu-blocklist'] });
const page = await browser.newPage({ viewport: { width: 1920, height: 1080 }, deviceScaleFactor: SCALE });
page.on('pageerror', (e) => console.error('page error:', e.message));
page.on('console', (m) => m.type() === 'error' && console.error('console:', m.text()));
const url = `${server.url}/${relative(ROOT, filmAbs).split('\\').join('/')}?autoplay=0`;
await page.goto(url, { waitUntil: 'load' });
await page.waitForFunction(() => window.__film && typeof window.__film.seek === 'function', null, { timeout: 30000 });
const { DUR, width = 1920, height = 1080 } = await page.evaluate(() => ({ DUR: window.__film.DUR, width: window.__film.width, height: window.__film.height }));
if (width !== 1920 || height !== 1080) await page.setViewportSize({ width, height });
await page.evaluate(() => window.__film.pause());
await page.evaluate(() => document.fonts.ready);

const end = TO ? Math.min(Number(TO), DUR) : DUR;
const frames = Math.round((end - FROM) * FPS);
if (STILLS) mkdirSync(STILLS, { recursive: true });

/* motion blur: n sub-frames per frame, piped at fps*n, averaged by tmix and
   thinned back to fps. One leading duplicate aligns the tmix window to each
   frame's own sub-frames: output frame k averages sub-frames kn+1 … kn+n. */
const vf = [];
if (SHUTTER > 1) vf.push(`tmix=frames=${SHUTTER}`, `framestep=${SHUTTER}`, 'trim=start_frame=1', 'setpts=PTS-STARTPTS');
vf.push('scale=in_range=full:out_range=tv', 'format=yuv420p');
const silent = `${tmp}.video.tmp.mp4`;
const ff = spawn('ffmpeg', ['-y', '-loglevel', 'error', '-f', 'image2pipe', '-framerate', String(FPS * SHUTTER), '-i', '-', '-vf', vf.join(','), '-r', String(FPS), '-c:v', 'libx264', '-preset', 'slow', '-crf', '16', '-profile:v', 'high', '-movflags', '+faststart', silent], { stdio: ['pipe', 'inherit', 'inherit'] });
const write = (buf) => new Promise((ok) => (ff.stdin.write(buf) ? ok() : ff.stdin.once('drain', ok)));
const shot = () => page.screenshot(PNG ? { type: 'png' } : { type: 'jpeg', quality: 95 });

/* if the page throws mid-render, stop ffmpeg instead of leaving it waiting on the pipe */
process.on('uncaughtException', (e) => (console.error('\n' + e.message), ff.kill('SIGKILL'), browser.close().finally(() => process.exit(1))));
const t0 = Date.now();
let first = true;
for (let i = 0; i < frames; i++) {
	const t = FROM + i / FPS;
	for (let k = 0; k < SHUTTER; k++) {
		/* sub-frames span the half frame AFTER the frame time, never before it, so a hard
		   cut placed on the frame grid stays a clean cut instead of a one-frame blend */
		const ts = SHUTTER > 1 ? t + (k / SHUTTER) * (0.5 / FPS) : t;
		await page.evaluate((v) => window.__film.seek(v), ts);
		const img = await shot();
		if (first && SHUTTER > 1) (await write(img), (first = false));
		await write(img);
		if (STILLS && k === Math.floor(SHUTTER / 2)) writeFileSync(join(STILLS, `f${String(i).padStart(5, '0')}_${t.toFixed(3)}.${PNG ? 'png' : 'jpg'}`), img);
	}
	if (i % FPS === 0) process.stdout.write(`\r  ${t.toFixed(1)}s / ${end.toFixed(1)}s  (${((Date.now() - t0) / 1000).toFixed(0)}s elapsed)`);
}
ff.stdin.end();
await new Promise((ok, fail) => ff.on('close', (c) => (c ? fail(new Error('ffmpeg exited ' + c)) : ok())));

let wav = null;
if (!AUDIO && (await page.evaluate(() => typeof window.__film.audioWav === 'function'))) {
	wav = `${tmp}.tmp.wav`;
	writeFileSync(wav, Buffer.from(await page.evaluate(() => window.__film.audioWav()), 'base64'));
}
await browser.close();
server.close();

const track = AUDIO ? resolve(AUDIO) : wav;
if (track) {
	const seek = FROM ? ['-ss', String(FROM)] : [];
	await run(['-i', silent, ...seek, '-i', track, '-map', '0:v', '-map', '1:a', '-c:v', 'copy', '-c:a', 'aac', '-b:a', '192k', '-shortest', '-movflags', '+faststart', OUT]);
} else await run(['-i', silent, '-c', 'copy', '-movflags', '+faststart', OUT]);
rmSync(silent);
if (wav) rmSync(wav);
console.log(`\n✓ ${OUT}  (${frames} frames, ${((Date.now() - t0) / 1000).toFixed(0)}s)`);

if (WEBM) {
	const webm = OUT.replace(/\.mp4$/, '.webm');
	await run(['-i', OUT, '-c:v', 'libvpx-vp9', '-b:v', '0', '-crf', '30', '-row-mt', '1', '-deadline', 'good', '-cpu-used', '2', '-pix_fmt', 'yuv420p', ...(track ? ['-c:a', 'libopus', '-b:a', '128k'] : ['-an']), webm]);
	console.log(`✓ ${webm}`);
}
