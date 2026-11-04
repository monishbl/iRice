/**
 * audio.js: sound design synthesised in code, rendered offline on the film's clock.
 *
 * A cue list is rendered with OfflineAudioContext into one buffer, so picture
 * and sound share a clock exactly. tools/render.mjs muxes it automatically when
 * the film exposes __film.audioWav(). For music, use a licensed track instead
 * (render.mjs --audio track.wav) and cut on its beats (recipes.beats()).
 *
 *   import { createSound } from './audio.js';
 *   const snd = createSound({ duration: f.DUR });
 *   snd.cue('whoosh', 3.2, 0.32);         // a whip
 *   snd.cue('click', 5.1);
 *   snd.cue('chime', 11.0, 0, 0.8);
 *   window.__film = { audioWav: () => snd.wav() };   // before f.start()
 *
 * Cue types and the motion they belong to (library/sound.md):
 *   whoosh  whip pans, fast flies           riser  push-ins, zoom-throughs (ends on the cut)
 *   swish   small objects in flight         thump  hard cuts, heavy landings
 *   pop     chips, icons, checks popping    click  cursor clicks, toggles
 *   tick    counters, reels, typing         chime  the brand moment (once, maybe twice)
 *   drone   a quiet bed under the whole film (cue it once at 0 with dur = duration)
 *
 * Deterministic: the noise comes from a seeded generator, so two renders are identical.
 */
import { rng } from './film.js';

export function createSound({ duration, sampleRate = 44100, seed = 3, master = 0.8 } = {}) {
	const CUES = [];
	const api = {
		/** @param {string} type @param {number} t start (s) @param {number} [dur] length (s) @param {number} [vol=1] */
		cue(type, t, dur = 0, vol = 1) {
			CUES.push([type, t, dur, vol]);
			return api;
		},
		cues: CUES,
		render,
		wav: async () => wavBase64(await render(), sampleRate)
	};

	async function render() {
		const SR = sampleRate;
		const ctx = new OfflineAudioContext(2, Math.ceil(SR * duration), SR);
		const out = ctx.createGain();
		out.gain.value = master;
		const comp = ctx.createDynamicsCompressor();
		out.connect(comp).connect(ctx.destination);
		const r = rng(seed);
		const noise = ctx.createBuffer(1, SR * 2, SR);
		const nd = noise.getChannelData(0);
		for (let i = 0; i < nd.length; i++) nd[i] = r() * 2 - 1;
		const env = (g, t, a, peak, d) => {
			g.gain.setValueAtTime(0.0001, t);
			g.gain.exponentialRampToValueAtTime(Math.max(0.0002, peak), t + a);
			g.gain.exponentialRampToValueAtTime(0.0001, t + a + d);
		};
		const src = (t, d) => {
			const s = ctx.createBufferSource();
			s.buffer = noise;
			s.loop = true;
			s.start(t);
			s.stop(t + d + 0.1);
			return s;
		};
		const osc = (type, f, t, d) => {
			const o = ctx.createOscillator();
			o.type = type;
			o.frequency.setValueAtTime(f, t);
			o.start(t);
			o.stop(t + d + 0.1);
			return o;
		};
		for (const [type, t0, d0, v] of CUES) {
			const t = Math.max(0, t0);
			const g = ctx.createGain();
			g.connect(out);
			if (type === 'whoosh' || type === 'swish') {
				const d = d0 || 0.3;
				const f = ctx.createBiquadFilter();
				f.type = 'bandpass';
				f.Q.value = type === 'swish' ? 6 : 1.4;
				const lo = type === 'swish' ? 1200 : 300;
				f.frequency.setValueAtTime(lo, t);
				f.frequency.exponentialRampToValueAtTime(lo * 6, t + d * 0.55);
				f.frequency.exponentialRampToValueAtTime(lo * 1.5, t + d);
				src(t, d).connect(f).connect(g);
				env(g, t, d * 0.5, (type === 'swish' ? 0.15 : 0.35) * v, d * 0.6);
			} else if (type === 'riser') {
				const d = d0 || 0.8;
				const f = ctx.createBiquadFilter();
				f.type = 'bandpass';
				f.Q.value = 3;
				f.frequency.setValueAtTime(400, t);
				f.frequency.exponentialRampToValueAtTime(5000, t + d);
				src(t, d).connect(f).connect(g);
				g.gain.setValueAtTime(0.0001, t);
				g.gain.exponentialRampToValueAtTime(0.3 * v, t + d);
				g.gain.exponentialRampToValueAtTime(0.0001, t + d + 0.08);
			} else if (type === 'thump') {
				const o = osc('sine', 90, t, 0.6);
				o.frequency.exponentialRampToValueAtTime(42, t + 0.35);
				o.connect(g);
				env(g, t, 0.01, 0.6 * v, 0.5);
			} else if (type === 'pop') {
				const o = osc('sine', 760, t, 0.15);
				o.frequency.exponentialRampToValueAtTime(380, t + 0.1);
				o.connect(g);
				env(g, t, 0.005, 0.12 * v, 0.12);
			} else if (type === 'click' || type === 'tick') {
				const f = ctx.createBiquadFilter();
				f.type = 'highpass';
				f.frequency.value = type === 'tick' ? 5000 : 2500;
				src(t, 0.04).connect(f).connect(g);
				env(g, t, 0.002, (type === 'tick' ? 0.15 : 0.35) * v, 0.035);
			} else if (type === 'chime') {
				[523.25, 783.99, 1046.5, 1567.98].forEach((fr, i) => {
					const o = osc('sine', fr, t + i * 0.06, 2.6), gg = ctx.createGain();
					o.connect(gg).connect(g);
					env(gg, t + i * 0.06, 0.01, 0.16 / (i + 1), 2.4);
				});
				g.gain.value = v;
			} else if (type === 'drone') {
				const d = d0 || duration;
				[110, 164.8, 220].forEach((fr, i) => {
					const o = osc('sine', fr, t, d), gg = ctx.createGain();
					gg.gain.setValueAtTime(0.0001, t);
					gg.gain.linearRampToValueAtTime((0.035 * v) / (i + 1), t + Math.min(2, d / 4));
					gg.gain.setValueAtTime((0.035 * v) / (i + 1), t + d - Math.min(2, d / 4));
					gg.gain.linearRampToValueAtTime(0.0001, t + d);
					o.connect(gg).connect(g);
				});
			} else console.warn('audio: unknown cue', type);
		}
		return ctx.startRendering();
	}
	return api;
}

/** 16-bit stereo WAV of an AudioBuffer, base64 (what render.mjs expects from __film.audioWav). */
export function wavBase64(buf, SR = buf.sampleRate) {
	const n = buf.length, ch = 2;
	const out = new DataView(new ArrayBuffer(44 + n * ch * 2));
	const w = (o, s) => [...s].forEach((c, i) => out.setUint8(o + i, c.charCodeAt(0)));
	w(0, 'RIFF');
	out.setUint32(4, 36 + n * ch * 2, true);
	w(8, 'WAVEfmt ');
	out.setUint32(16, 16, true);
	out.setUint16(20, 1, true);
	out.setUint16(22, ch, true);
	out.setUint32(24, SR, true);
	out.setUint32(28, SR * ch * 2, true);
	out.setUint16(32, ch * 2, true);
	out.setUint16(34, 16, true);
	w(36, 'data');
	out.setUint32(40, n * ch * 2, true);
	const L = buf.getChannelData(0), R = buf.numberOfChannels > 1 ? buf.getChannelData(1) : L;
	const c = (v) => Math.max(-1, Math.min(1, v));
	for (let i = 0, o = 44; i < n; i++, o += 4) (out.setInt16(o, c(L[i]) * 0x7fff, true), out.setInt16(o + 2, c(R[i]) * 0x7fff, true));
	let s = '';
	const u = new Uint8Array(out.buffer);
	for (let i = 0; i < u.length; i += 0x8000) s += String.fromCharCode(...u.subarray(i, i + 0x8000));
	return btoa(s);
}
