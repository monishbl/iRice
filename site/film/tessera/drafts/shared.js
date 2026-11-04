/*
 * Shared by the Tessera films (a.html "Two halves", b.html "Tiles"): footage as
 * <video> kept in step with the film's clock, the composited screens (a 2x
 * still of the real desktop with the recorded region live on top, at the place
 * it was recorded from), views into them, the titles and the Badge.
 */
import { keyed, EASE, clamp } from '../../engine/film.js';

export const W = 1920, H = 1080;
export const ACCENT = '#FFB45C';
export const BADGE = ''; /* Phosphor circle-wavy (filled): the Badge */

/* Default to looping, as the site plays it (?loop=0 to stop at the end) */
export function loopByDefault() {
	const q = new URLSearchParams(location.search);
	if (!q.has('loop')) {
		q.set('loop', '1');
		history.replaceState(null, '', '?' + q.toString());
	}
}

/*
 * A <video> kept in step with the film: film time → clip time through `map`
 * (piecewise linear [[filmT, clipT], …]: speed-ups and holds) or `at` + `rate`.
 * While the film plays, the video plays at the segment's rate and is re-synced
 * when it drifts; when scrubbed or seeked, it's set exactly, and seek() waits
 * for it (settle).
 */
export function vclip(f, v, { at = 0, rate = 1, map = null, from = -0.6, until = Infinity }) {
	v = typeof v === 'string' ? f.$(v) : v;
	v.muted = true;
	v.playsInline = true;
	v.preload = 'auto';
	v.disableRemotePlayback = true;
	const pts = map || [[at, 0], [at + 3600, 3600 * rate]];
	const clipAt = (t) => {
		if (t <= pts[0][0]) return { c: pts[0][1], k: 0 };
		for (let i = 1; i < pts.length; i++) {
			const [t0, c0] = pts[i - 1], [t1, c1] = pts[i];
			if (t <= t1) return { c: c0 + ((t - t0) / (t1 - t0)) * (c1 - c0), k: (c1 - c0) / (t1 - t0) };
		}
		const [tl, cl] = pts[pts.length - 1];
		return { c: cl + (t - tl), k: 1 };
	};
	const t0 = pts[0][0] + from;
	let last = -1;
	f.fx((t) => {
		const live = t >= t0 && t < until;
		const playing = t > last && t - last < 0.25;
		last = t;
		if (!live) {
			if (!v.paused) v.pause();
			return;
		}
		const { c, k } = clipAt(t);
		const target = Math.max(0, Math.min(c, (v.duration || 1e9) - 0.05));
		if (playing && k > 0.05) {
			if (Math.abs(v.playbackRate - k) > 0.01) v.playbackRate = Math.min(16, Math.max(0.0625, k));
			if (Math.abs(v.currentTime - target) > 0.12) v.currentTime = target;
			if (v.paused) v.play().catch(() => {});
		} else {
			if (!v.paused) v.pause();
			if (Math.abs(v.currentTime - target) > 0.02) v.currentTime = target;
		}
	});
	f.settle((t) => {
		if (t < t0 || t >= until) return null;
		return new Promise((res) => {
			if (!v.seeking && v.readyState >= 2) return res();
			const done = () => res();
			v.addEventListener('seeked', done, { once: true });
			v.addEventListener('loadeddata', done, { once: true });
			setTimeout(done, 2000);
		});
	});
	return v;
}

/*
 * A real screen: the plate (a 2x still, shown at 1920x1080) and, optionally, a
 * recorded region live on top at the place it was recorded from.
 *   screen(parent, { plate, clip, region: [x, y, w, h] })  → { el, video }
 */
export function screen(parent, { plate, clip = null, region = null, cls = '' }) {
	const el = document.createElement('div');
	el.className = 'scr ' + cls;
	el.style.cssText = `position:absolute;left:0;top:0;width:${W}px;height:${H}px;transform-origin:0 0;overflow:hidden`;
	if (plate) {
		const img = document.createElement('img');
		img.src = plate;
		img.style.cssText = `position:absolute;left:0;top:0;width:${W}px;height:${H}px`;
		el.appendChild(img);
	}
	let video = null;
	if (clip) {
		video = document.createElement('video');
		video.src = clip;
		const [x, y, w, h] = region || [0, 0, W, H];
		video.style.cssText = `position:absolute;left:${x}px;top:${y}px;width:${w}px;height:${h}px;object-fit:fill`;
		el.appendChild(video);
	}
	parent.appendChild(el);
	return { el, video };
}

/*
 * A view into a screen: keys of { cx, cy, s } (the screen point at the
 * container's centre, and the zoom), into a container of size cw x ch.
 */
export function view(f, scrEl, cw, ch, keys) {
	const K = keyed(keys);
	f.fx((t) => {
		const { cx, cy, s } = K(t);
		scrEl.style.transform = `translate(${cw / 2 - cx * s}px, ${ch / 2 - cy * s}px) scale(${s})`;
	});
}

/* A title: words rise out of blur one by one, the accent word last; out by `out` */
export function title(f, el, t, { out = null, step = 0.08, dur = 0.55, rise = 26, blur = 14, accent = null } = {}) {
	el = typeof el === 'string' ? f.$(el) : el;
	const words = el.textContent.trim().split(/\s+/);
	el.textContent = '';
	const spans = words.map((w, i) => {
		const s = document.createElement('span');
		s.textContent = w;
		s.style.display = 'inline-block';
		s.style.marginRight = i < words.length - 1 ? '0.26em' : '0';
		if (accent !== null && (accent === i || (accent === -1 && i === words.length - 1))) s.style.color = ACCENT;
		el.appendChild(s);
		return s;
	});
	spans.forEach((s, i) => {
		const t0 = t + i * step;
		const keys = [[0, { dy: rise, o: 0, blur }], [t0, { dy: rise, o: 0, blur }], [t0 + dur, { dy: 0, o: 1, blur: 0 }, 'o']];
		if (out !== null) keys.push([out, { dy: 0, o: 1, blur: 0 }], [out + 0.35, { dy: -14, o: 0, blur: 8 }, 'i']);
		f.tw(s, keys);
	});
	return spans;
}

/* Small helpers */
export const ease = EASE;
export function ramp01(t, a, b) {
	return clamp((t - a) / (b - a), 0, 1);
}
