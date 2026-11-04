/**
 * recipes.js: the library's moves as functions that add keys to a film.
 *
 * Each recipe is a named, measured pattern from library/transitions.md or
 * library/type.md, with its defaults taken from the reference films. A recipe
 * only adds keys (and sometimes an fx), so it composes with hand-written
 * tw()/cam() calls. Call recipes after the keys they build on (whip() reads
 * the camera where it is at t0).
 *
 * Element modes. An element with class "a" is placed by its centre (x, y)
 * in world pixels, so a recipe that moves it needs to know where it is:
 * pass x / y. Anything else (a span from split(), a row in a list) is
 * "in flow" and is moved by offsets (dx, dy, sc) that leave layout alone.
 *
 *   import * as R from './recipes.js';
 *   R.blurIn(f, '#hero', 1.2);
 *   R.zoomThrough(f, 3.0, 3.6, 812, 540);   // into the counter of the "o"
 */
import { split, lerp, ramp, EASE } from './film.js';

const el = (f, s) => (typeof s === 'string' ? f.$(s) : s);
const isAbs = (e) => e.classList.contains('a');
const K = (e) => (isAbs(e) ? { s: 's', x: 'x', y: 'y' } : { s: 'sc', x: 'dx', y: 'dy' });

/* ── timing helpers ── */

/** Beat times for a tempo: beats(120, 0, 8) → [0, .5, 1, …]. Cut on these and the film feels scored, even silent. */
export const beats = (bpm, from, count, sub = 1) => Array.from({ length: count }, (_, i) => from + (i * 60) / bpm / sub);

/**
 * Accelerating montage times: holds shrink geometrically from `first` to `last` seconds.
 * The "rev-out" before a logo (Gemini Omni, Google Vids: 600 ms → 100 ms).
 */
export function accelerate(t0, n, first = 0.6, last = 0.1) {
	const r = n > 1 ? Math.pow(last / first, 1 / (n - 1)) : 1;
	const out = [];
	let t = t0;
	for (let i = 0; i < n; i++) (out.push(t), (t += first * r ** i));
	return out;
}

/* ── entrances ── */

/** Blur-to-sharp with a slight scale-down: the emphasis entrance. 0.25–0.35 s, ease-out. */
export function blurIn(f, sel, t, { dur = 0.32, s0 = 1.18, blur = 14, rise = 0, y, ease = 'o' } = {}) {
	const e = el(f, sel), k = K(e);
	const a = { o: 0, [k.s]: s0, blur }, b = { o: 1, [k.s]: 1, blur: 0 };
	if (rise) isAbs(e) ? ((a.y = y + rise), (b.y = y)) : ((a.dy = rise), (b.dy = 0));
	f.tw(e, [[t, a], [t + dur, b, ease]]);
}

/** Pop in with a ~10% overshoot. Chips, icons, badges, checks. 0.3–0.4 s. */
export function pop(f, sel, t, { dur = 0.36, from = 0.4 } = {}) {
	const e = el(f, sel), k = K(e);
	f.tw(e, [[t, { o: 0, [k.s]: from }], [t + dur * 0.4, { o: 1 }, 'o'], [t + dur, { [k.s]: 1 }, 'back']]);
}

/** Rack-focus re-entry: arrive defocused and a little low, ease to sharp. The cheap, classy cut (Arrow). */
export function rackIn(f, sel, t, { dur = 0.45, blur = 20, drop = 60, y } = {}) {
	const e = el(f, sel), k = K(e);
	const yy0 = isAbs(e) ? y + drop : drop, yy1 = isAbs(e) ? y : 0;
	f.tw(e, [[t, { o: 0, blur, [k.y]: yy0 }], [t + dur * 0.5, { o: 1 }, 'o'], [t + dur, { blur: 0, [k.y]: yy1 }, 'o']]);
}

/** Masked rise per word: the word slides up out of an overflow:hidden line box. Use on split() spans inside a .mask parent. */
export function maskRise(f, spans, t, { step = 0.06, dur = 0.42, ease = 'expo' } = {}) {
	spans.forEach((s, i) => f.tw(s, [[t + i * step, { dy: s.offsetHeight * 1.1 }], [t + i * step + dur, { dy: 0 }, ease]]));
}

/**
 * Word append with live re-centring (AI Studio, Apple, Credsvault). The line
 * is centred on cx; each word fades up at its time and the line glides so
 * the visible words stay centred. Needs a .a line with data-anchor="left"
 * whose words are split() spans, after fonts have loaded.
 */
export function wordAppend(f, lineSel, words, times, { cx = 960, y = 540, dur = 0.22, rise = 10 } = {}) {
	const line = el(f, lineSel);
	line.dataset.anchor = 'left';
	const right = words.map((w) => w.offsetLeft + w.offsetWidth);
	const xs = right.map((r) => cx - r / 2);
	const keys = [[times[0], { x: xs[0], y }]];
	for (let i = 1; i < words.length; i++) keys.push([times[i], { x: xs[i - 1] }], [times[i] + dur, { x: xs[i] }, 'o']);
	f.tw(line, keys);
	words.forEach((w, i) => f.tw(w, [[times[i], { o: 0, dy: rise }], [times[i] + dur, { o: 1, dy: 0 }, 'o']]));
}

/**
 * A title, word by word: each word rises 30 px out of a 12 px blur (expo out), 0.1 s apart,
 * with one accent word (the last, by default). The workhorse title of a launch film: used
 * for every chapter line, so the viewer learns it. Pass `out` to blur it away again.
 * Splits the element itself, so call it after fonts load. Returns the word spans.
 */
export function titleIn(f, sel, t, { x, y, step = 0.1, dur = 0.5, rise = 30, blur = 12, accent = -1, accentClass = 'acc', out, outDur = 0.3 } = {}) {
	const e = el(f, sel);
	const words = split(e, 'words');
	if (accent !== null) words.at(accent)?.classList.add(accentClass);
	if (x !== undefined) f.tw(e, [[0, { x, y }]]);
	words.forEach((w, i) => f.tw(w, [[t + i * step, { o: 0, dy: rise, blur }], [t + i * step + dur, { o: 1, dy: 0, blur: 0 }, 'expo']]));
	if (out !== undefined) f.tw(e, [[out - outDur, { o: 1, blur: 0 }], [out, { o: 0, blur: 10 }, 'i']]);
	f.show(e, t, out ?? Infinity);
	return words;
}

/* ── exits ── */

/** Smear out: stretch along the direction of travel with motion blur and leave in 3–6 frames. Cut on the blurriest frame. */
export function smearOut(f, sel, t, { dur = 0.16, dist = 480, dir = 1, mb = 60, x } = {}) {
	const e = el(f, sel), k = K(e);
	const x0 = isAbs(e) ? x : 0;
	f.tw(e, [[t, { [k.x]: x0, sx: 1, mbx: 0, o: 1 }], [t + dur, { [k.x]: x0 + dir * dist, sx: 1.6, mbx: mb, o: 0 }, 'i']]);
}

/** Depth push: the previous line steps back (down, smaller, blurred, dimmer) while the next one lands above (Credsvault). */
export function depthPush(f, sel, t, { dur = 0.22, drop = 44, s = 0.85, blur = 6, o = 0.45, y } = {}) {
	const e = el(f, sel), k = K(e);
	const y0 = isAbs(e) ? y : 0;
	f.tw(e, [[t, { [k.y]: y0, [k.s]: 1, blur: 0, o: 1 }], [t + dur, { [k.y]: y0 + drop, [k.s]: s, blur, o }, 'o']]);
}

/* ── masks ── */

/** Circle iris on an element: radius r0 → r1 px, centred at (ix, iy) % of the element. Close on a logo, open on a scene. */
export function iris(f, sel, t0, t1, r0, r1, { ix = 50, iy = 50, ease = 'io' } = {}) {
	f.tw(el(f, sel), [[t0, { ir: r0, ix, iy }], [t1, { ir: r1 }, ease]]);
}

/** Reveal left → right (clip inset). The wordmark wipe, a gradient fill on one word. */
export function wipeIn(f, sel, t, { dur = 0.4, ease = 'io' } = {}) {
	f.tw(el(f, sel), [[t, { clip: 0 }], [t + dur, { clip: 100 }, ease]]);
}

/* ── camera ── */

/**
 * Zoom through a point: an accelerating push (exponential ease-in) until a
 * detail fills the frame, usually the counter of a letter or a button's fill.
 * Cut to the next scene on the frame where the field is a flat colour, and
 * start that scene on the same colour.
 */
export function zoomThrough(f, t0, t1, px, py, { to = 40, ease = 'expoIn', centre = true } = {}) {
	f.zoomAbout(t0, t1, px, py, to, ease, { centre });
}

/** Whip pan: the camera travels (dx, dy) in `dur` with a fast middle, and the world gets motion blur along the move. */
export function whip(f, t0, { dur = 0.32, dx = 1920, dy = 0, mb = 90 } = {}) {
	const c = f.camAt(t0);
	f.cam([[t0, { cx: c.cx, cy: c.cy }], [t0 + dur, { cx: c.cx + dx, cy: c.cy + dy }, 'io5']]);
	const mx = Math.abs(dx) > Math.abs(dy) ? mb : 0, my = mx ? 0 : mb;
	f.tw(f.W, [[t0, { mbx: 0, mby: 0 }], [t0 + dur / 2, { mbx: mx, mby: my }, 'i'], [t0 + dur, { mbx: 0, mby: 0 }, 'o']]);
}

/** Hold-drift: the camera creeps in a few percent across a hold so no frame is ever frozen. */
export function holdDrift(f, t0, t1, { ds = 0.04 } = {}) {
	const c = f.camAt(t0);
	f.cam([[t0, { s: c.s }], [t1, { s: c.s * (1 + ds) }, 'lin']]);
}

/** Punch-in: hard cut (no tween) to the camera framing a detail at scale s. The macro insert before a hero click. */
export function punchIn(f, t, cx, cy, s) {
	const c = f.camAt(t - 1e-4);
	f.cam([[t - 1e-4, { cx: c.cx, cy: c.cy, s: c.s }], [t, { cx, cy, s }, 'step']]);
}

/* ── shared elements ── */

/**
 * FLIP a box from one rect to another (world px, {cx, cy, w, h} from f.box()).
 * The shared-element match cut: logo → app slot, chip → sheet cell, pill → Gantt bar.
 * The element needs explicit width/height styling and class "a".
 */
export function flip(f, sel, t0, t1, from, to, { ease = 'io', br } = {}) {
	const a = { x: from.cx, y: from.cy, w: from.w, h: from.h }, b = { x: to.cx, y: to.cy, w: to.w, h: to.h };
	if (br) ((a.br = br[0]), (b.br = br[1]));
	f.tw(el(f, sel), [[t0, a], [t1, b, ease]]);
}

/* ── interaction ── */

/** A cursor click: the cursor dips, the target presses to 97%, and an optional ring ripples out. */
export function click(f, cursorSel, t, { target, ring, ringSize = 2.6 } = {}) {
	const c = el(f, cursorSel), kc = K(c);
	f.tw(c, [[t - 0.06, { [kc.s]: 1 }], [t, { [kc.s]: 0.86 }, 'o'], [t + 0.12, { [kc.s]: 1 }, 'o']]);
	if (target) {
		const g = el(f, target), kg = K(g);
		f.tw(g, [[t - 0.04, { [kg.s]: 1 }], [t + 0.03, { [kg.s]: 0.97 }, 'o'], [t + 0.2, { [kg.s]: 1 }, 'o']]);
	}
	if (ring) {
		const r = el(f, ring), kr = K(r);
		f.tw(r, [[t, { o: 0, [kr.s]: 0.3 }], [t + 0.02, { o: 0.9 }], [t + 0.5, { o: 0, [kr.s]: ringSize }, 'o']]);
	}
}

/**
 * Slot reel / rolodex / picker wheel. `reel` is an in-flow column of equal-height
 * rows inside a fixed window; the reel steps one row at each time, and every row's
 * scale, opacity and blur follow its distance from the selected slot (Credsvault,
 * Apple's roster, LangEase's language wheel, Linear's wheel picker).
 */
export function slotReel(f, reelSel, times, { dur = 0.3, rowH, ease = 'io', fade = 0.55, blur = 5, shrink = 0.25, start = 0 } = {}) {
	const reel = el(f, reelSel);
	const rows = [...reel.children];
	const h = rowH || rows[0].offsetHeight;
	const E = typeof ease === 'function' ? ease : null;
	return f.fx((t) => {
		let c = start;
		for (const at of times) {
			const p = Math.max(0, Math.min(1, (t - at) / dur));
			c += E ? E(p) : p < 0.5 ? 4 * p * p * p : 1 - Math.pow(-2 * p + 2, 3) / 2;
		}
		reel.style.transform = `translateY(${(-c * h).toFixed(2)}px)`;
		rows.forEach((r, i) => {
			const d = Math.min(Math.abs(i - c), 3);
			r.style.opacity = Math.max(0, 1 - d * fade).toFixed(3);
			r.style.filter = d > 0.02 ? `blur(${(d * blur).toFixed(2)}px)` : '';
			r.style.transform = `scale(${(1 - Math.min(d, 1) * shrink).toFixed(4)})`;
		});
	});
}

/**
 * Jump cuts on a beat: the same shot reframed by a hard step in scale at each time
 * (1 → 1.05 → 1 → …), so each new answer or state lands with a cut, not a fade.
 */
export function jumpCuts(f, sels, times, { s = 1.05, from = 1 } = {}) {
	const keys = [];
	times.forEach((t, i) => {
		const a = i % 2 ? s : from, b = i % 2 ? from : s;
		keys.push([t - 0.001, { s: a }], [t, { s: b }, 'step']);
	});
	for (const x of Array.isArray(sels) ? sels : [sels]) {
		const e = el(f, x);
		f.tw(e, isAbs(e) ? keys : keys.map(([t, v, z]) => [t, { sc: v.s }, z]));
	}
}

/**
 * Fly an element along an arc from a to b (points, or functions of t for moving ends):
 * a file sent from a phone to a laptop. It rises by `lift` px at mid-flight, scales
 * s0 → s1, tilts by `spin` degrees and back, and fades in and out at its ends. The
 * element needs class "a"; its rest transform is replaced while it flies.
 */
export function arc(f, sel, t0, t1, a, b, { lift = 260, spin = 0, s0 = 1, s1 = 0.5, ease = 'io', fade = 0.15 } = {}) {
	const e = el(f, sel);
	const P = (p, t) => (typeof p === 'function' ? p(t) : p);
	const E = typeof ease === 'function' ? ease : EASE[ease];
	f.show(e, t0, t1);
	return f.fx((t) => {
		if (t < t0 || t >= t1) return;
		const p = E(ramp(t, t0, t1)), A = P(a, t), B = P(b, t);
		const x = lerp(A[0], B[0], p), y = lerp(A[1], B[1], p) - lift * Math.sin(Math.PI * p);
		const s = lerp(s0, s1, p), r = spin * Math.sin(Math.PI * p);
		e.style.transform = `translate(${x.toFixed(2)}px,${y.toFixed(2)}px) translate(-50%,-50%) scale(${s.toFixed(4)}) rotate(${r.toFixed(2)}deg)`;
		e.style.opacity = (Math.min(1, ramp(t, t0, t0 + fade)) * (1 - ramp(t, t1 - fade, t1))).toFixed(3);
	});
}

/** Hard-cut a property through a list of values at given times (colour-flip cuts, background stutter). */
export function cuts(f, sel, times, prop, values) {
	f.tw(el(f, sel), times.map((t, i) => [t, { [prop]: values[i] }, 'step']));
}

/** Show each element for one slot in turn: a strobe montage, or the background-swap stutter under a locked foreground. */
export function strobe(f, sels, times) {
	sels.forEach((s, i) => f.show(el(f, s), times[i], times[i + 1] ?? times[i] + 0.15));
}
