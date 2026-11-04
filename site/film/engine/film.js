/**
 * film.js: a seekable timeline engine for product films rendered from code.
 *
 * The one rule: every frame is a pure function of t. Nothing reads the wall
 * clock, nothing is random unless seeded, and nothing accumulates state
 * between frames. That is what lets tools/render.mjs seek to any frame,
 * screenshot it and get the same picture every time. A screen recording drops
 * frames under load and cannot do this.
 *
 *   import { createFilm } from './film.js';
 *   const f = createFilm({ duration: 12 });
 *   f.tw('#title', [[0, { o: 0, y: 580 }], [0.6, { o: 1, y: 540 }, 'o']]);
 *   f.cam([[0, { cx: 960, cy: 540, s: 1 }], [3, { s: 1.08 }, 'lin']]);
 *   f.start();
 *
 * Markup contract (engine/template/film.html sets this up):
 *   #art    the 1920×1080 frame, scaled to fit the window
 *   #world  the camera's world: cam() transforms it. Put scenes inside.
 *   .a      absolutely positioned at the world origin; tracks place its centre
 *           at (x, y). With data-anchor="left", x is the left edge.
 *
 * What this deliberately does not do: no physics, no timeline GUI and no
 * dependency. GSAP or anime.js can drive the same DOM if you prefer them;
 * they are seekable too (gsap timeline.seek(t), anime.seek(ms)). Hand either
 * one the same t from render().
 */

/* ── maths ── */
export const clamp = (v, a = 0, b = 1) => Math.max(a, Math.min(b, v));
/** 0 → 1 as t goes from a to b, clamped. The workhorse of every FX function. */
export const ramp = (t, a, b) => (b === a ? (t >= b ? 1 : 0) : clamp((t - a) / (b - a)));
export const lerp = (a, b, p) => a + (b - a) * p;
/** A 0 → 1 → 0 bump between a and b, peaking at the middle. */
export const bump = (t, a, b) => Math.sin(Math.PI * ramp(t, a, b));

/** CSS cubic-bezier(x1, y1, x2, y2) as an easing function. */
export function cubicBezier(x1, y1, x2, y2) {
	const cx = 3 * x1, bx = 3 * (x2 - x1) - cx, ax = 1 - cx - bx;
	const cy = 3 * y1, by = 3 * (y2 - y1) - cy, ay = 1 - cy - by;
	const X = (u) => ((ax * u + bx) * u + cx) * u;
	const Y = (u) => ((ay * u + by) * u + cy) * u;
	const dX = (u) => (3 * ax * u + 2 * bx) * u + cx;
	return (p) => {
		if (p <= 0 || p >= 1) return p;
		let u = p;
		for (let i = 0; i < 8; i++) {
			const e = X(u) - p, d = dX(u);
			if (Math.abs(e) < 1e-6) break;
			if (Math.abs(d) < 1e-6) break;
			u -= e / d;
		}
		let lo = 0, hi = 1;
		if (Math.abs(X(u) - p) > 1e-4) for (let i = 0; i < 30; i++) ((u = (lo + hi) / 2), X(u) < p ? (lo = u) : (hi = u));
		return Y(u);
	};
}

/**
 * Easing names usable as the third element of any key. Measured against the
 * reference films (library/motion-and-easing.md): entrances are strong
 * ease-outs ('o', 'expo'), zoom-throughs ease in ('i', 'expoIn'), camera
 * moves are symmetric ('io', 'io5'), loops are linear.
 */
export const EASE = {
	lin: (p) => p,
	/** cubic in-out: the default */
	io: (p) => (p < 0.5 ? 4 * p * p * p : 1 - Math.pow(-2 * p + 2, 3) / 2),
	/** quintic in-out: whips and long camera moves, with a fast middle */
	io5: (p) => (p < 0.5 ? 16 * p ** 5 : 1 - Math.pow(-2 * p + 2, 5) / 2),
	/** quart out: the standard entrance */
	o: (p) => 1 - Math.pow(1 - p, 4),
	o2: (p) => 1 - (1 - p) * (1 - p),
	expo: (p) => (p === 1 ? 1 : 1 - Math.pow(2, -10 * p)),
	/** quart in: exits, the run-up of a zoom-through */
	i: (p) => p ** 4,
	i3: (p) => p ** 3,
	expoIn: (p) => (p === 0 ? 0 : Math.pow(2, 10 * p - 10)),
	/** overshoot by about 10% and settle: pops, chips, icons */
	back: (p) => 1 + 2.70158 * Math.pow(p - 1, 3) + 1.70158 * Math.pow(p - 1, 2),
	/** elastic settle: use once per film, if at all */
	el: (p) => (p === 0 || p === 1 ? p : Math.pow(2, -10 * p) * Math.sin((p * 10 - 0.75) * ((2 * Math.PI) / 3)) + 1),
	bounce: (p) => {
		const n = 7.5625, d = 2.75;
		if (p < 1 / d) return n * p * p;
		if (p < 2 / d) return n * (p -= 1.5 / d) * p + 0.75;
		if (p < 2.5 / d) return n * (p -= 2.25 / d) * p + 0.9375;
		return n * (p -= 2.625 / d) * p + 0.984375;
	},
	/** hold the old value until the key, then jump: a hard cut inside a track */
	step: (p) => (p < 1 ? 0 : 1),
	/** Material 3 "emphasized" and the like, as named beziers */
	emph: cubicBezier(0.2, 0, 0, 1),
	snap: cubicBezier(0.7, 0, 0.84, 0),
	/** one move through measured keys: speed carries through each (see smoothAt) */
	smooth: Object.assign((p) => p, { smooth: true })
};
const easeOf = (e) => (typeof e === 'function' ? e : EASE[e] || EASE.io);

/** Seeded PRNG (mulberry32). Math.random() would give every render a different film. */
export function rng(seed = 1) {
	let a = seed >>> 0;
	return () => {
		a = (a + 0x6d2b79f5) >>> 0;
		let x = a;
		x = Math.imul(x ^ (x >>> 15), x | 1);
		x ^= x + Math.imul(x ^ (x >>> 7), x | 61);
		return ((x ^ (x >>> 14)) >>> 0) / 4294967296;
	};
}

/* ── colour ── */
const hex = (s) => {
	let h = s.slice(1);
	if (h.length === 3) h = [...h].map((c) => c + c).join('');
	return [0, 2, 4].map((i) => parseInt(h.slice(i, i + 2), 16)).concat(h.length === 8 ? parseInt(h.slice(6, 8), 16) / 255 : 1);
};
const mixAny = (a, b, p) => {
	if (typeof a === 'number') return lerp(a, b, p);
	if (typeof a === 'string' && a[0] === '#') {
		const A = hex(a), B = hex(b);
		const c = A.map((v, i) => lerp(v, B[i], p));
		return `rgba(${c[0].toFixed(0)},${c[1].toFixed(0)},${c[2].toFixed(0)},${c[3].toFixed(3)})`;
	}
	return p < 1 ? a : b;
};

/** Value of a sorted key list [[t, v, ease], …] at time t. */
/**
 * A run of keys eased 'smooth' is one move through measured points, not a stop at each: a
 * monotone cubic (PCHIP, Fritsch–Carlson) through the run, so the speed carries through every
 * key, nothing overshoots, and the curve still passes exactly through each measurement. Keys
 * chained with 'o' or 'io' instead come to rest at every key, which reads as a stutter.
 */
function smoothAt(pts, i, t) {
	const T = (k) => pts[k][0], V = (k) => pts[k][1];
	const inRun = (k) => k >= 1 && k < pts.length && pts[k][2].smooth && typeof V(k) === 'number' && typeof V(k - 1) === 'number';
	const d = (k) => (V(k) - V(k - 1)) / (T(k) - T(k - 1));
	/* slope at key k from the run's segments either side of it */
	const slope = (k) => {
		const L = inRun(k), R = inRun(k + 1);
		if (L && R) {
			const d1 = d(k), d2 = d(k + 1), h1 = T(k) - T(k - 1), h2 = T(k + 1) - T(k);
			if (d1 * d2 <= 0) return 0;
			return (3 * (h1 + h2)) / ((2 * h2 + h1) / d1 + (h2 + 2 * h1) / d2);
		}
		/* the end of a run: the one-sided three-point estimate, kept the sign of its segment */
		const [a, b] = L ? [k, k - 1] : [k + 1, k + 2];
		const h1 = Math.abs(T(a) - T(a - 1));
		if (!inRun(b) || b < 1 || b >= pts.length) return d(a);
		const d1 = d(a), d2 = d(b), h2 = Math.abs(T(b) - T(b - 1));
		let m = ((2 * h1 + h2) * d1 - h1 * d2) / (h1 + h2);
		if (m * d1 <= 0) m = 0;
		else if (d1 * d2 <= 0 && Math.abs(m) > Math.abs(3 * d1)) m = 3 * d1;
		return m;
	};
	const h = T(i) - T(i - 1), s = clamp((t - T(i - 1)) / h);
	const s2 = s * s, s3 = s2 * s;
	return (2 * s3 - 3 * s2 + 1) * V(i - 1) + (s3 - 2 * s2 + s) * h * slope(i - 1) + (-2 * s3 + 3 * s2) * V(i) + (s3 - s2) * h * slope(i);
}

/**
 * A key the value keeps moving through (the same way before and after) is a point on one move,
 * not a stop: eased into ('o', 'io', 'i') it would come to rest there and set off again, a
 * stutter. Such keys join a 'smooth' run, so the speed carries through. A hold (the same value)
 * or a turn (back the other way) keeps its eases; 'step' keys are cuts and stay.
 */
function joinMoves(pts) {
	const num = (k) => typeof pts[k][1] === 'number';
	const out = pts.map((p) => [...p]);
	for (let k = 1; k < pts.length - 1; k++) {
		if (!num(k - 1) || !num(k) || !num(k + 1)) continue;
		if (pts[k][2] === EASE.step || pts[k + 1][2] === EASE.step) continue;
		const d1 = pts[k][1] - pts[k - 1][1], d2 = pts[k + 1][1] - pts[k][1];
		if (d1 * d2 > 0) out[k][2] = out[k + 1][2] = EASE.smooth;
	}
	return out;
}

/**
 * Measured keys carry measuring error, and a curve through every one of them faithfully
 * replays it: a 2 px reversal mid-move is a visible bounce. Within each 'smooth' run, drop an
 * interior key that turns the motion back by less than 4% of the run's range (noise), until
 * none is left; real turns (a card that dips and returns) are far larger and stay.
 */
function denoiseSmooth(pts) {
	let out = pts;
	for (let pass = 0; pass < 20; pass++) {
		let changed = false;
		for (let k = 1; k < out.length - 1; k++) {
			const p = out;
			if (!(p[k][2].smooth && p[k + 1][2].smooth && typeof p[k][1] === 'number' && typeof p[k - 1][1] === 'number' && typeof p[k + 1][1] === 'number')) continue;
			/* the run this key belongs to, for its range */
			let a = k, b = k;
			while (a > 0 && p[a][2].smooth && typeof p[a - 1][1] === 'number') a--;
			while (b + 1 < p.length && p[b + 1][2].smooth && typeof p[b + 1][1] === 'number') b++;
			const vs = p.slice(a, b + 1).map((x) => x[1]);
			const eps = 0.04 * (Math.max(...vs) - Math.min(...vs));
			const d1 = p[k][1] - p[k - 1][1], d2 = p[k + 1][1] - p[k][1];
			if (d1 * d2 < 0 && Math.min(Math.abs(d1), Math.abs(d2)) < eps) {
				out = [...p.slice(0, k), ...p.slice(k + 1)];
				changed = true;
				break;
			}
		}
		if (!changed) break;
	}
	return out;
}

export function valueAt(pts, t) {
	if (!pts.length) return undefined;
	if (t <= pts[0][0]) return mixAny(pts[0][1], pts[0][1], 1);
	for (let i = 1; i < pts.length; i++)
		if (t <= pts[i][0]) {
			if (pts[i][2].smooth && typeof pts[i][1] === 'number' && typeof pts[i - 1][1] === 'number') return smoothAt(pts, i, t);
			return mixAny(pts[i - 1][1], pts[i][1], pts[i][2](ramp(t, pts[i - 1][0], pts[i][0])));
		}
	return mixAny(pts.at(-1)[1], pts.at(-1)[1], 1);
}

/**
 * A keyframed function of plain numbers, for things the tracks don't own (a device's
 * position and scale driven in an fx, a split-screen panel's rect, a shader's state).
 * Keys are [t, {name: value}, ease]; a key inherits every value it doesn't name from the
 * key before it, so `[12.2, {}]` is a hold. `s` interpolates geometrically, so a zoom
 * reads even all the way.
 *   const lap = keyed([[0, { x: 960, s: 4.8 }], [10.6, {}], [12.2, { s: 0.9 }, 'io']]);
 *   f.fx((t) => place(el, lap(t)));
 */
export function keyed(K) {
	const full = [];
	for (const [t, v, e] of K) full.push([t, { ...(full.at(-1)?.[1] ?? {}), ...v }, easeOf(e)]);
	return (t) => {
		if (t <= full[0][0]) return { ...full[0][1] };
		for (let i = 1; i < full.length; i++) {
			const [t0, a] = full[i - 1], [t1, b, e] = full[i];
			if (t < t1) {
				const p = e(ramp(t, t0, t1)), o = {};
				for (const k in b) o[k] = !(k in a) ? b[k] : k === 's' && a[k] > 0 && b[k] > 0 ? a[k] * Math.pow(b[k] / a[k], p) : lerp(a[k], b[k], p);
				return o;
			}
		}
		return { ...full.at(-1)[1] };
	};
}

/**
 * Film time → footage time, piecewise linear through [[filmT, clipT], …]. Trim, hold and
 * speed up recorded footage without re-encoding it: a pointer's slow travel at 2×, the
 * wait for a result cut out. Before the first point it holds; after the last it plays on
 * at 1× (or holds, with { after: 'hold' }).
 *   remap([[14.2, 0], [15.6, 1.4], [17.7, 5.6], [18.7, 6.6], [18.7, 8.7]])
 */
export function remap(points, { after = 'play' } = {}) {
	const P = [...points].sort((a, b) => a[0] - b[0]);
	return (t) => {
		if (t <= P[0][0]) return P[0][1];
		for (let i = 1; i < P.length; i++) if (t < P[i][0]) return lerp(P[i - 1][1], P[i][1], (t - P[i - 1][0]) / (P[i][0] - P[i - 1][0]));
		const [tl, cl] = P.at(-1);
		return after === 'hold' ? cl : cl + (t - tl);
	};
}

/** The 1-based frame number shown at local time `local` (s) of an n-frame clip at fps. Clamped. */
export const frameIndex = (local, fps, n) => clamp(Math.floor(local * fps + 1e-4) + 1, 1, n);

/**
 * Load a captured clip's clip.json (tools/capture/web.mjs or frames.sh) for f.footage().
 * `url` is the clip's folder, relative to the film page.
 *   const tap = await loadClip('capture/clips/new-project');
 *   f.footage('#screen', tap, { at: 4.6 });
 *   const click = tap.events.find((e) => e.type === 'click');   // cut on it, aim the cursor at it
 */
export async function loadClip(url) {
	const r = await fetch(`${url}/clip.json`);
	if (!r.ok) throw new Error(`loadClip: no ${url}/clip.json (run the capture, or frames.sh, first)`);
	return { ...(await r.json()), url };
}

/** Whether a list of [t0, t1) ranges covers t. An element shown by several show() calls is visible in any of them. */
export const within = (ranges, t) => ranges.some(([a, b]) => t >= a && t < b);

const TRANSFORM = ['x', 'y', 's', 'sx', 'sy', 'r', 'rx', 'ry', 'z', 'skx'];
const FLOW = ['dx', 'dy', 'sc'];

/**
 * @param {object} o
 * @param {number} o.duration seconds
 * @param {number} [o.width=1920]
 * @param {number} [o.height=1080]
 * @param {string|Element} [o.world='#world']
 * @param {string|Element} [o.art='#art']
 */
export function createFilm({ duration, width = 1920, height = 1080, world = '#world', art = '#art' } = {}) {
	const $ = (s) => (typeof s === 'string' ? document.querySelector(s) : s);
	/** a selector, an element, or an array of either → elements */
	const all = (sel) => (Array.isArray(sel) ? sel : [sel]).flatMap((s) => (typeof s === 'string' ? [...document.querySelectorAll(s)] : [s]));
	const W = $(world);
	/* the camera maths assume the world is transformed about its top-left corner */
	if (W) W.style.transformOrigin = '0 0';
	const ART = $(art);
	const TRACKS = [];
	const FX = [];
	const SHOW = [];
	const CAM = { cx: [], cy: [], s: [], r: [] };
	const CAMFN = [];
	const SETTLE = [];
	const SHAKES = [];
	let shakeFn = () => 0;
	let hooks = [];

	/**
	 * Add keys to an element's track. A second call on the same element merges.
	 * Each prop interpolates only between the keys that name it, so a prop
	 * that must HOLD through a gap has to be named again at the end of the gap,
	 * or it drifts all the way to its next key. Most timeline bugs are this one.
	 * @param {string|Element|Array<string|Element>} sel
	 * @param {Array<[number, object, (string|Function)?]>} keys [t, {prop: value}, ease into this key]
	 */
	function tw(sel, keys) {
		const els = all(sel);
		if (!els.length) console.warn('tw: nothing matches', sel);
		for (const el of els) {
			let tr = TRACKS.find((x) => x.el === el);
			if (!tr) TRACKS.push((tr = { el, P: {} }));
			for (const [t, props, e] of keys) for (const [k, v] of Object.entries(props)) (tr.P[k] ||= []).push([t, v, easeOf(e)]);
			for (const k in tr.P) tr.P[k] = denoiseSmooth(joinMoves(tr.P[k].sort((a, b) => a[0] - b[0])));
		}
		return api;
	}

	/**
	 * Visible only while t0 <= t < t1 (display: none outside). Keeps heavy scenes out of layout.
	 * Call it again for another range: the element shows in any of them.
	 */
	function show(sel, t0, t1 = Infinity) {
		for (const el of all(sel)) {
			let s = SHOW.find((x) => x[0] === el);
			if (!s) SHOW.push((s = [el, []]));
			s[1].push([t0, t1]);
		}
		return api;
	}

	/**
	 * Register fn(t) → a promise the renderer waits for after each seek, before its
	 * screenshot: an image to decode, a video to seek, a texture to upload. The player
	 * doesn't wait; only seek() does.
	 */
	function settle(fn) {
		SETTLE.push(fn);
		return api;
	}

	/**
	 * Play recorded footage on an <img>, one frame at a time, so every seek shows the exact
	 * frame (a <video>'s currentTime is not frame-exact, and seeking it is slow).
	 * `clip` is the clip.json a capture tool wrote ({ dir, fps, n }), or that object by hand.
	 *   at:   the film time the clip's first frame plays at (default 0), or
	 *   map:  film time → clip time, for trims and speed changes (see remap())
	 *   from, to: only drive the image in this range, so one <img> can play a montage
	 *         of clips, a footage() call per shot
	 * Frames are <dir>/0001.jpg …; pass `name: (i) => url` for anything else.
	 */
	function footage(sel, clip, { at = 0, map, from = -Infinity, to = Infinity, name } = {}) {
		const el = $(sel);
		const { fps = 30, n, ext = 'jpg' } = clip;
		const dir = clip.url ?? clip.dir;
		const local = map ?? ((t) => t - at);
		const url = name ?? ((i) => `${dir}/${String(i).padStart(4, '0')}.${ext}`);
		if (!n) console.warn('footage: the clip has no frame count (n)', clip);
		if (!el.dataset.footage) {
			el.dataset.footage = '1';
			settle(() => (el.getAttribute('src') ? el.decode().catch(() => {}) : null));
		}
		return fx((t) => {
			if (t < from || t >= to) return;
			const u = url(frameIndex(local(t), fps, n));
			if (el.getAttribute('src') !== u) el.setAttribute('src', u);
		});
	}

	/** Camera keys: cx, cy = the world point at the frame centre; s = zoom; r = roll (deg). s interpolates in log space, so a zoom feels even. */
	function cam(keys) {
		for (const [t, p, e] of keys) for (const [k, v] of Object.entries(p)) CAM[k].push([t, k === 's' ? Math.log(v) : v, easeOf(e)]);
		for (const k in CAM) CAM[k] = joinMoves(CAM[k].sort((a, b) => a[0] - b[0]));
		return api;
	}
	/** Replace the camera between t0 and t1 with a function of t → {cx, cy, s, r}. See zoomAbout(). */
	function camFn(t0, t1, fn) {
		CAMFN.push([t0, t1, fn]);
		return api;
	}
	/**
	 * Zoom into world point (px, py), scale going to s1, with the point gliding to
	 * the frame centre as it grows (centre: false keeps it where it is on screen).
	 * This is the zoom-through. Interpolating cx and s as separate keys would make
	 * the target drift, because a point's screen position depends on both.
	 */
	function zoomAbout(t0, t1, px, py, s1, ease = 'i', { centre = true } = {}) {
		const E = easeOf(ease);
		let start;
		return camFn(t0, t1, (t) => {
			start ||= camAt(t0 - 1e-6);
			const p = ramp(t, t0, t1);
			const s = Math.exp(lerp(Math.log(start.s), Math.log(s1), E(p)));
			/* the point's offset from the frame centre, in screen px: shrinks to 0 over the move */
			const k = centre ? 1 - EASE.io(p) : 1;
			const dx = (px - start.cx) * start.s * k, dy = (py - start.cy) * start.s * k;
			return { cx: px - dx / s, cy: py - dy / s, s, r: start.r };
		});
	}
	/** A decaying shake starting at t: amplitude in px, duration in s. */
	function shake(t, amp = 8, dur = 0.45) {
		SHAKES.push([t, amp, dur]);
		return api;
	}
	/** Continuous extra shake, amp(t) in px. */
	function shakeWith(fn) {
		shakeFn = fn;
		return api;
	}
	/** Run fn(t) on every render, after tracks. For procedural motion, counters, typing, canvases, WebGL. */
	function fx(fn) {
		FX.push(fn);
		return api;
	}

	/**
	 * Play a keyframed timeline (tools/match/camfit.py) and return the values at t. Each key is
	 * [t, values, ease]: ease shapes the move INTO that key: [x1, y1, x2, y2] (cubic-bezier),
	 * an EASE name, or 'step' (the value jumps at t). Before the first key / after the last, holds.
	 */
	function keysAt(K, t) {
		/* 'smooth' keys: one monotone cubic per value through the keys (as camfit.py verified them) */
		if (K.some((k) => k[2] === 'smooth')) {
			if (!K._dims) K._dims = K[0][1].map((_, j) => K.map((k) => [k[0], k[1][j], k[2] === 'step' ? EASE.step : easeOf(k[2])]));
			return K._dims.map((pts) => valueAt(pts, t));
		}
		if (!K._ez) K._ez = K.map((k) => (Array.isArray(k[2]) ? cubicBezier(...k[2]) : k[2] === 'step' ? null : easeOf(k[2])));
		if (t <= K[0][0]) return K[0][1];
		const last = K[K.length - 1];
		if (t >= last[0]) return last[1];
		let i = 1;
		while (K[i][0] < t) i++;
		const a = K[i - 1], b = K[i], ez = K._ez[i];
		if (!ez) return a[1];
		const p = ez((t - a[0]) / (b[0] - a[0]));
		return a[1].map((v, j) => v + (b[1][j] - v) * p);
	}

	/**
	 * Move a layer by a camera timeline from camfit.py: keyframes of where the anchor frame's
	 * four corners land on screen, each move with its fitted ease. The layer is laid out in
	 * anchor-frame screen coordinates, as for homography(). This is the one to use: playing the
	 * raw per-frame track (homography()) replays its measurement noise, which reads as a bounce.
	 * @param {string|Element} sel
	 * @param {{w: number, h: number, keys: any[]}} data
	 * @param {{shift?: number, from?: number, to?: number}} [o]  as for homography()
	 */
	function camKeys(sel, data, { shift = 0, from = -Infinity, to = Infinity } = {}) {
		const el = $(sel);
		el.style.transformOrigin = '0 0';
		return fx((t) => {
			if (t < from || t >= to) return;
			const c = keysAt(data.keys, t - shift);
			/* the keys are where the box (data.x, data.y, w × h, anchor-frame px) lands; the layer's own origin is 0,0 */
			el.style.transform = quad(data.w, data.h, [[c[0], c[1]], [c[2], c[3]], [c[4], c[5]], [c[6], c[7]]]) + (data.x || data.y ? ` translate(${-(data.x || 0)}px,${-(data.y || 0)}px)` : '');
		});
	}

	/**
	 * Follow a camera move transcribed from a reference (tools/match/camtrack.py): one
	 * homography per frame, mapping the anchor frame's screen pixels to frame t's. The
	 * element is laid out in anchor-frame screen coordinates (a 1920×1080 layer at the
	 * frame's top-left); this sets its transform to the frame's matrix3d, interpolating
	 * between samples. Outside [first, last] it holds the end values.
	 * @param {string|Element} sel
	 * @param {{frames: number[][]}} data  [[t, h11 … h33], …]
	 * @param {{shift?: number, from?: number, to?: number}} [o]  shift: seconds added to every sample time;
	 *   from / to: apply only within [from, to), so tracks can own the element outside it
	 */
	function homography(sel, data, { shift = 0, from = -Infinity, to = Infinity } = {}) {
		const el = $(sel);
		const F = data.frames;
		el.style.transformOrigin = '0 0';
		return fx((t) => {
			/* outside [from, to) the element's own tracks own its transform */
			if (t < from || t >= to) return;
			const u = t - shift;
			let i = 0;
			if (u <= F[0][0]) i = 0;
			else if (u >= F[F.length - 1][0]) i = F.length - 2;
			else while (i < F.length - 2 && F[i + 1][0] < u) i++;
			const a = F[i], b = F[Math.min(i + 1, F.length - 1)];
			const p = b[0] === a[0] ? 0 : clamp((u - a[0]) / (b[0] - a[0]));
			const h = [];
			for (let k = 1; k <= 9; k++) h.push(lerp(a[k], b[k], p));
			const n = h[8];
			const [A, B, C, D, E, F2, G, H] = h.map((v) => v / n);
			el.style.transform = `matrix3d(${A},${D},0,${G},${B},${E},0,${H},0,0,1,0,${C},${F2},0,1)`;
		});
	}

	/**
	 * A CSS matrix3d that maps a w×h element (transform-origin 0 0) onto a screen quad: four
	 * corners [[x, y] × 4] in order top-left, top-right, bottom-right, bottom-left. For posing a
	 * flat element exactly as a tilted plane appears in a reference frame.
	 */
	function quad(w, h, Q) {
		const src = [[0, 0], [w, 0], [w, h], [0, h]];
		const A = [], b = [];
		for (let i = 0; i < 4; i++) {
			const [x, y] = src[i], [u, v] = Q[i];
			A.push([x, y, 1, 0, 0, 0, -u * x, -u * y]); b.push(u);
			A.push([0, 0, 0, x, y, 1, -v * x, -v * y]); b.push(v);
		}
		/* Gaussian elimination, 8×8 */
		for (let c = 0; c < 8; c++) {
			let p = c;
			for (let r = c + 1; r < 8; r++) if (Math.abs(A[r][c]) > Math.abs(A[p][c])) p = r;
			[A[c], A[p]] = [A[p], A[c]];
			[b[c], b[p]] = [b[p], b[c]];
			for (let r = 0; r < 8; r++) {
				if (r === c) continue;
				const k = A[r][c] / A[c][c];
				for (let j = c; j < 8; j++) A[r][j] -= k * A[c][j];
				b[r] -= k * b[c];
			}
		}
		const [a0, a1, a2, a3, a4, a5, a6, a7] = b.map((v, i) => v / A[i][i]);
		return `matrix3d(${a0},${a3},0,${a6},${a1},${a4},0,${a7},0,0,1,0,${a2},${a5},0,1)`;
	}

	function camAt(t, skipFn = false) {
		if (!skipFn) for (const [a, b, fn] of CAMFN) if (t >= a && t <= b) return fn(t);
		const g = (k, d) => (CAM[k].length ? valueAt(CAM[k], t) : d);
		return { cx: g('cx', width / 2), cy: g('cy', height / 2), s: Math.exp(g('s', 0)), r: g('r', 0) };
	}
	function applyCam(t) {
		const c = camAt(t);
		let sx = 0, sy = 0;
		for (const [at, k, d] of SHAKES) {
			const p = (t - at) / d;
			if (p > 0 && p < 1) ((sx += Math.sin(p * 47) * k * (1 - p) ** 2), (sy += Math.cos(p * 39) * k * (1 - p) ** 2));
		}
		const e = shakeFn(t);
		sx += Math.sin(t * 61) * e;
		sy += Math.cos(t * 53) * e;
		W.style.transform = `translate(${(width / 2 + sx).toFixed(2)}px,${(height / 2 + sy).toFixed(2)}px) rotate(${c.r.toFixed(3)}deg) scale(${c.s.toFixed(5)}) translate(${(-c.cx).toFixed(2)}px,${(-c.cy).toFixed(2)}px)`;
	}

	let mbN = 0;
	function motionBlur(el, x, y) {
		if (!el.__mb) {
			let svg = document.getElementById('__mb');
			if (!svg) {
				svg = document.createElementNS('http://www.w3.org/2000/svg', 'svg');
				svg.id = '__mb';
				svg.setAttribute('style', 'position:absolute;width:0;height:0');
				document.body.appendChild(svg);
			}
			const id = `mb${mbN++}`;
			svg.insertAdjacentHTML('beforeend', `<filter id="${id}" x="-50%" y="-50%" width="200%" height="200%" color-interpolation-filters="sRGB"><feGaussianBlur stdDeviation="0 0"/></filter>`);
			el.__mb = { id, node: svg.lastChild.firstChild };
		}
		el.__mb.node.setAttribute('stdDeviation', `${Math.max(0, x).toFixed(1)} ${Math.max(0, y).toFixed(1)}`);
		return `url(#${el.__mb.id})`;
	}

	function applyTracks(t) {
		for (const { el, P } of TRACKS) {
			const v = {};
			for (const k in P) v[k] = valueAt(P[k], t);
			const st = el.style;
			if (TRANSFORM.some((k) => k in v)) {
				const s = v.s ?? 1;
				const anc = el.dataset.anchor === 'left' ? '0,-50%' : el.dataset.anchor === 'topleft' ? '0,0' : '-50%,-50%';
				const persp = 'rx' in v || 'ry' in v || 'z' in v ? `perspective(${el.dataset.persp || 1600}px) ` : '';
				st.transform = `${persp}translate(${(v.x ?? 0).toFixed(2)}px,${(v.y ?? 0).toFixed(2)}px) translate(${anc})` + ('z' in v ? ` translateZ(${v.z.toFixed(1)}px)` : '') + ` rotate(${(v.r ?? 0).toFixed(3)}deg)` + ('rx' in v ? ` rotateX(${v.rx.toFixed(3)}deg)` : '') + ('ry' in v ? ` rotateY(${v.ry.toFixed(3)}deg)` : '') + ('skx' in v ? ` skewX(${v.skx.toFixed(2)}deg)` : '') + ` scale(${(s * (v.sx ?? 1)).toFixed(5)},${(s * (v.sy ?? 1)).toFixed(5)})`;
			} else if (FLOW.some((k) => k in v)) st.transform = `translate(${(v.dx ?? 0).toFixed(2)}px,${(v.dy ?? 0).toFixed(2)}px) scale(${(v.sc ?? 1).toFixed(5)})`;
			if ('w' in v) st.width = v.w.toFixed(1) + 'px';
			if ('h' in v) st.height = v.h.toFixed(1) + 'px';
			if ('br' in v) st.borderRadius = v.br.toFixed(1) + 'px';
			if ('bw' in v) st.borderWidth = v.bw.toFixed(2) + 'px';
			if ('ls' in v) st.letterSpacing = v.ls.toFixed(3) + 'em';
			if ('bg' in v) st.background = v.bg;
			if ('fg' in v) st.color = v.fg;
			if ('o' in v) {
				const o = clamp(v.o);
				st.opacity = o.toFixed(3);
				st.visibility = o < 0.002 ? 'hidden' : '';
			}
			if ('blur' in v || 'mbx' in v || 'mby' in v || 'bri' in v || 'sat' in v) {
				const f = [];
				if ((v.mbx ?? 0) > 0.05 || (v.mby ?? 0) > 0.05) f.push(motionBlur(el, v.mbx ?? 0, v.mby ?? 0));
				if ((v.blur ?? 0) > 0.05) f.push(`blur(${v.blur.toFixed(2)}px)`);
				if ('bri' in v) f.push(`brightness(${v.bri.toFixed(3)})`);
				if ('sat' in v) f.push(`saturate(${v.sat.toFixed(3)})`);
				st.filter = f.join(' ');
			}
			if ('ir' in v) st.clipPath = `circle(${Math.max(0, v.ir).toFixed(2)}px at ${(v.ix ?? 50).toFixed(2)}% ${(v.iy ?? 50).toFixed(2)}%)`;
			else if ('clip' in v) st.clipPath = `inset(-20% ${(100 - v.clip).toFixed(3)}% -20% 0)`;
			else if ('ct' in v || 'cr' in v || 'cb' in v || 'cl' in v) st.clipPath = `inset(${(v.ct ?? 0).toFixed(3)}% ${(v.cr ?? 0).toFixed(3)}% ${(v.cb ?? 0).toFixed(3)}% ${(v.cl ?? 0).toFixed(3)}%${'crad' in v ? ` round ${v.crad.toFixed(1)}px` : ''})`;
			for (const k in v) if (k.startsWith('--')) st.setProperty(k, typeof v[k] === 'number' ? v[k].toFixed(4) : v[k]);
		}
	}

	let drawn = NaN;
	function render(t) {
		for (const [el, R] of SHOW) {
			const d = within(R, t) ? '' : 'none';
			if (el.style.display !== d) el.style.display = d;
		}
		applyCam(t);
		applyTracks(t);
		for (const f of FX) f(t);
		for (const h of hooks) h(t);
		drawn = t;
	}

	/** A box in world coordinates, from layout (transforms ignored). For FLIP-style shared-element moves. */
	function box(el) {
		el = $(el);
		let x = 0, y = 0, n = el;
		while (n && n !== W) ((x += n.offsetLeft), (y += n.offsetTop), (n = n.offsetParent));
		const w = el.offsetWidth, h = el.offsetHeight;
		return { x, y, w, h, cx: x + w / 2, cy: y + h / 2 };
	}

	/**
	 * Where an element really is at time t, in world coordinates: renders t, reads
	 * the on-screen rect and undoes the camera. Unlike box(), this includes every
	 * transform, so it is the one to use for zoom targets and match cuts.
	 */
	function boxAt(el, t) {
		el = $(el);
		const keep = drawn;
		render(t);
		const r = el.getBoundingClientRect(), a = ART.getBoundingClientRect();
		const k = a.width / width;
		const c = camAt(t);
		const toWorld = (sx, sy) => [((sx - a.left) / k - width / 2) / c.s + c.cx, ((sy - a.top) / k - height / 2) / c.s + c.cy];
		const [x0, y0] = toWorld(r.left, r.top), [x1, y1] = toWorld(r.right, r.bottom);
		if (!Number.isNaN(keep)) render(keep);
		return { x: x0, y: y0, w: x1 - x0, h: y1 - y0, cx: (x0 + x1) / 2, cy: (y0 + y1) / 2 };
	}

	/* ── playback: ?t=12.5 starts there, ?autoplay=0 pauses, ?loop=1 loops, ?player=1 shows controls ── */
	function start() {
		const q = new URLSearchParams(location.search);
		const LOOP = q.get('loop') === '1';
		let t = Number(q.get('t') || 0), playing = q.get('autoplay') !== '0', last = 0;
		const ui = document.getElementById('player');
		if (q.get('player') === '1' && ui) ui.hidden = false;
		const scrub = ui?.querySelector('input[type=range]');
		const time = ui?.querySelector('[data-time]');
		const btn = ui?.querySelector('button');
		if (scrub) ((scrub.max = duration), (scrub.step = 1 / 60), (scrub.oninput = (e) => (t = Number(e.target.value))));
		const icon = () => btn && (btn.textContent = playing ? 'Pause' : 'Play');
		const toggle = () => ((playing = !playing), playing && t >= duration && (t = 0), icon());
		if (btn) btn.onclick = toggle;
		hooks.push((tt) => {
			if (time) time.textContent = `${tt.toFixed(2)} / ${duration.toFixed(2)}`;
			if (scrub && document.activeElement !== scrub) scrub.value = tt;
		});
		addEventListener('keydown', (e) => {
			if (e.code === 'Space') (e.preventDefault(), toggle());
			if (e.key === 'ArrowRight') t = Math.min(duration, t + (e.shiftKey ? 1 / 30 : 0.5));
			if (e.key === 'ArrowLeft') t = Math.max(0, t - (e.shiftKey ? 1 / 30 : 0.5));
			if (e.key === 'r') ((t = 0), (playing = true), icon());
			if (e.key === 'h' && ui) ui.hidden = !ui.hidden;
		});
		document.addEventListener('visibilitychange', () => (last = 0));
		const fit = () => ART && (ART.style.transform = `translate(-50%,-50%) scale(${Math.min(innerWidth / width, innerHeight / height).toFixed(5)})`);
		addEventListener('resize', fit);
		fit();
		icon();
		const frame = (now) => {
			if (playing) {
				t += last ? (now - last) / 1000 : 0;
				if (t >= duration) LOOP ? (t %= duration) : ((t = duration), (playing = false), icon());
			}
			last = now;
			/* draw only when the clock moved: an idle page that re-renders every frame
			   (a WebGL scene especially) starves the renderer's screenshots */
			if (t !== drawn) render(t);
			requestAnimationFrame(frame);
		};
		/* the renderer's contract: DUR, seek(t) (draws synchronously; returns a promise), pause() */
		window.__film = {
			...(window.__film || {}) /* keeps an audioWav() set before start() */,
			DUR: duration,
			width,
			height,
			/* draws synchronously, then resolves once footage and anything else registered
			   with settle() is ready: the renderer awaits it before each screenshot */
			seek: (v) => {
				t = v;
				render(v);
				return Promise.all(SETTLE.map((fn) => fn(v)));
			},
			pause: () => ((playing = false), icon())
		};
		requestAnimationFrame(frame);
		return api;
	}

	const api = { DUR: duration, width, height, W, tw, show, cam, camFn, zoomAbout, shake, shakeWith, fx, homography, camKeys, keysAt, quad, render, box, boxAt, start, camAt, settle, footage, $ };
	return api;
}

/* ── text helpers ── */

/**
 * Wrap each word or character of el in an inline-block span and return them.
 * Animate the spans with flow props (dx, dy, sc, o, blur), which leave layout alone.
 */
export function split(el, by = 'words') {
	const text = el.textContent;
	el.textContent = '';
	const parts = by === 'chars' ? [...text] : text.split(/(\s+)/);
	const out = [];
	for (const p of parts) {
		if (by === 'words' && /^\s+$/.test(p)) {
			el.append(p);
			continue;
		}
		const s = document.createElement('span');
		s.className = by === 'chars' ? 'ch' : 'wd';
		s.style.display = 'inline-block';
		s.style.whiteSpace = 'pre';
		s.textContent = p;
		el.append(s);
		out.push(s);
	}
	return out;
}

/**
 * Typewriter as a function of t: returns an fx() that shows the first n characters.
 * cps ≈ 10–15 for a readable hero prompt, 25+ for small UI text.
 * A caret element (optional) blinks at 2 Hz while idle and holds solid while typing.
 */
export function typer(el, text, t0, cps = 14, { caret = null, from = 0 } = {}) {
	const t1 = t0 + (text.length - from) / cps;
	return (t) => {
		const n = t < t0 ? from : Math.min(text.length, from + Math.floor((t - t0) * cps));
		if (el.textContent !== text.slice(0, n)) el.textContent = text.slice(0, n);
		if (caret) caret.style.opacity = t >= t0 && t <= t1 + 0.05 ? 1 : Math.floor(t * 2) % 2 === 0 ? 1 : 0;
	};
}

/** Count from a to b between t0 and t1 with an ease; fmt formats the number. Ease-out so it slows into the final value. */
export function counter(el, a, b, t0, t1, { ease = 'o', fmt = (v) => Math.round(v).toLocaleString('en-US') } = {}) {
	const E = easeOf(ease);
	return (t) => {
		const s = fmt(lerp(a, b, E(ramp(t, t0, t1))));
		if (el.textContent !== s) el.textContent = s;
	};
}

/**
 * Scramble-decode: random glyphs resolving left to right into text (the "code" reveal).
 * Deterministic: the glyphs depend on the frame index, not Math.random().
 */
export function scramble(el, text, t0, t1, { glyphs = '{}[]<>/=+*#%$&01', fps = 30, seed = 7 } = {}) {
	return (t) => {
		if (t < t0) return void (el.textContent = '');
		const p = ramp(t, t0, t1);
		const fixed = Math.floor(p * text.length);
		const r = rng(seed + Math.floor(t * fps));
		let s = text.slice(0, fixed);
		for (let i = fixed; i < text.length; i++) s += text[i] === ' ' ? ' ' : glyphs[Math.floor(r() * glyphs.length)];
		if (el.textContent !== s) el.textContent = s;
	};
}
