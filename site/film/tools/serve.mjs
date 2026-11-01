#!/usr/bin/env node
/**
 * serve.mjs: a static file server for previewing films.
 *
 * Films import the engine as ES modules, and browsers refuse module imports
 * from file:// URLs, so a film has to be opened over HTTP. This serves a
 * folder with no dependencies and no caching (so a reload always shows the
 * latest edit).
 *
 *   node tools/serve.mjs [root=.] [--port 4173]
 *
 * render.mjs uses the same server internally.
 */
import { createServer } from 'node:http';
import { createReadStream, statSync } from 'node:fs';
import { extname, join, normalize, resolve } from 'node:path';

const TYPES = {
	'.html': 'text/html; charset=utf-8',
	'.js': 'text/javascript; charset=utf-8',
	'.mjs': 'text/javascript; charset=utf-8',
	'.css': 'text/css; charset=utf-8',
	'.json': 'application/json',
	'.svg': 'image/svg+xml',
	'.png': 'image/png',
	'.jpg': 'image/jpeg',
	'.jpeg': 'image/jpeg',
	'.webp': 'image/webp',
	'.avif': 'image/avif',
	'.gif': 'image/gif',
	'.woff2': 'font/woff2',
	'.woff': 'font/woff',
	'.ttf': 'font/ttf',
	'.otf': 'font/otf',
	'.mp4': 'video/mp4',
	'.webm': 'video/webm',
	'.wav': 'audio/wav',
	'.mp3': 'audio/mpeg',
	'.glb': 'model/gltf-binary',
	'.gltf': 'model/gltf+json',
	'.hdr': 'application/octet-stream',
	'.wasm': 'application/wasm'
};

/** Start a server on root; resolves to { url, close }. Port 0 picks a free one. */
export function serve(root = '.', port = 0) {
	const base = resolve(root);
	const server = createServer((req, res) => {
		let path = decodeURIComponent(new URL(req.url, 'http://x').pathname);
		let file = normalize(join(base, path));
		if (!file.startsWith(base)) return void (res.writeHead(403), res.end());
		try {
			if (statSync(file).isDirectory()) file = join(file, 'index.html');
			const size = statSync(file).size;
			res.writeHead(200, { 'content-type': TYPES[extname(file).toLowerCase()] || 'application/octet-stream', 'content-length': size, 'cache-control': 'no-store' });
			createReadStream(file).pipe(res);
		} catch {
			res.writeHead(404, { 'content-type': 'text/plain' });
			res.end(`404 ${path}`);
		}
	});
	return new Promise((ok) => server.listen(port, '127.0.0.1', () => ok({ url: `http://127.0.0.1:${server.address().port}`, close: () => server.close() })));
}

if (import.meta.url === `file://${process.argv[1]}`) {
	const args = process.argv.slice(2);
	const pi = args.indexOf('--port');
	const port = pi >= 0 ? Number(args.splice(pi, 2)[1]) : 4173;
	const { url } = await serve(args[0] || '.', port);
	console.log(`serving ${resolve(args[0] || '.')} at ${url}/`);
}
