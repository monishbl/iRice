/*
 * The film's liquid glass: one WebGL pass over a full-frame canvas, a pure
 * function of the state it's given (so every frame renders the same way).
 *
 *   - two images, A and B, with a liquid wipe from A to B (a wavy front that
 *     bends what's under it and splits it into colour), or a shockwave ring
 *     that opens B out of A from a point (the "impact");
 *   - a glass body over them: up to four droplets merging (smooth union)
 *     into a rounded chip, refracting the image at its bevel, with
 *     dispersion (each colour bent by a different amount), a soft frost,
 *     a rim of light and a shadow;
 *   - letters inside the chip, raised in the glass (a blurred text mask
 *     used as height), so the word appears as refraction before it's light.
 *
 *   const g = await createGlass(canvas, { images: { petals: 'assets/p1.jpg', … } });
 *   g.draw({ a: 'petals', b: 'ocean', wipe: 0.4, … });
 */

const VERT = `
attribute vec2 p;
void main() { gl_Position = vec4(p, 0.0, 1.0); }
`;

const FRAG = `
precision highp float;
uniform vec2 uRes;
uniform sampler2D uA, uB, uText;
uniform float uT;
uniform float uWipe;      // < 0: off; 0..1 the front's progress from A to B
uniform vec2 uWipeDir;    // the direction the front travels
uniform vec4 uShock;      // centre x, y (px), radius (px), strength
uniform vec4 uBall0, uBall1, uBall2, uBall3; // x, y, r, on
uniform vec4 uRect;       // centre x, y, half w, half h (px)
uniform float uRectR, uRectOn, uGlass, uDisp, uText1, uWob, uBright, uKA, uKB;
uniform vec2 uPanA, uPanB;

float sdBox(vec2 p, vec2 b, float r) {
	vec2 q = abs(p) - b + r;
	return length(max(q, 0.0)) + min(max(q.x, q.y), 0.0) - r;
}
float smin(float a, float b, float k) {
	float h = clamp(0.5 + 0.5 * (b - a) / k, 0.0, 1.0);
	return mix(b, a, h) - k * h * (1.0 - h);
}
float ball(vec2 p, vec4 b) { return b.w > 0.5 ? length(p - b.xy) - b.z : 1e5; }
// the glass body's signed distance, in px
float body(vec2 p) {
	float d = 1e5;
	d = smin(d, ball(p, uBall0), 70.0);
	d = smin(d, ball(p, uBall1), 70.0);
	d = smin(d, ball(p, uBall2), 70.0);
	d = smin(d, ball(p, uBall3), 70.0);
	if (uRectOn > 0.001) {
		vec2 hb = max(uRect.zw * uRectOn, vec2(1.0));
		d = smin(d, sdBox(p - uRect.xy, hb, min(uRectR, min(hb.x, hb.y))), 90.0);
	}
	// liquid: the surface still settling
	d += uWob * (5.0 * sin(p.x * 0.019 + uT * 3.1) * sin(p.y * 0.023 - uT * 2.3) + 3.0 * sin(p.x * 0.041 - uT * 4.7));
	return d;
}
// an image, covering the frame, slightly zoomed and panned (Ken Burns)
vec3 img(sampler2D s, vec2 px, float k, vec2 pan) {
	vec2 uv = (px - uRes * 0.5) / (uRes * k) + 0.5 + pan;
	uv = clamp(uv, vec2(0.001), vec2(0.999));
	return texture2D(s, uv).rgb; // (rows are uploaded top first, and px.y counts from the top)
}
// what's behind the glass at px: A, B, and the front or ring between them,
// which bends the image along its edge (off: offset in px, applied per colour)
vec3 scene(vec2 px, float split) {
	vec3 a, b;
	if (uShock.w > 0.0) {
		vec2 dv = px - uShock.xy;
		float r = length(dv);
		float e = r - uShock.z;               // < 0 inside the ring: B
		float band = exp(-e * e / (2.0 * 70.0 * 70.0));
		vec2 dir = r > 0.0 ? dv / r : vec2(0.0);
		vec2 off = dir * band * uShock.w * (120.0 + split * 40.0);
		a = img(uA, px - off, uKA, uPanA);
		b = img(uB, px - off * 0.6, uKB, uPanB);
		float m = smoothstep(18.0, -18.0, e);
		return mix(a, b, m) + band * uShock.w * 0.18;
	}
	if (uWipe >= 0.0) {
		vec2 c = px - uRes * 0.5;
		float along = dot(c, uWipeDir) / (abs(uWipeDir.x) * uRes.x + abs(uWipeDir.y) * uRes.y) + 0.5;
		vec2 side = vec2(-uWipeDir.y, uWipeDir.x);
		float s = dot(c, side);
		float front = uWipe * 1.5 - 0.25 + 0.035 * sin(s * 0.009 + uT * 5.0) + 0.02 * sin(s * 0.023 - uT * 7.0);
		float e = (along - front) * 1400.0;   // px-ish from the front, + ahead
		float band = exp(-e * e / (2.0 * 90.0 * 90.0));
		vec2 off = uWipeDir * band * (95.0 + split * 22.0); // (gentler than the impact: text crosses it)
		a = img(uA, px + off, uKA, uPanA);
		b = img(uB, px + off * 0.7, uKB, uPanB);
		float m = smoothstep(20.0, -20.0, e);
		return mix(a, b, m) + band * 0.12;
	}
	return img(uA, px, uKA, uPanA);
}
vec3 sceneRGB(vec2 px, vec2 off, float disp) {
	// dispersion: red bends least, blue most
	return vec3(
		scene(px + off * (1.0 - disp), -1.0).r,
		scene(px + off, 0.0).g,
		scene(px + off * (1.0 + disp), 1.0).b);
}
void main() {
	vec2 px = vec2(gl_FragCoord.x, uRes.y - gl_FragCoord.y);
	float d = body(px);
	// the background, split into colour wherever a front or ring passes
	// (scene() splits the colours itself, only in the band a front or ring passes through)
	vec3 col = sceneRGB(px, vec2(0.0), 0.0);
	if (uGlass > 0.001 && d < 120.0) {
		// height: rises over the bevel; letters rise inside it
		float bevel = 54.0;
		float dx = body(px + vec2(1.5, 0.0)) - d, dy = body(px + vec2(0.0, 1.5)) - d;
		vec2 g = vec2(dx, dy) / 1.5;          // points outward
		float hb = smoothstep(0.0, bevel, -d);
		vec2 slope = -g * (1.0 - hb) * 1.6;   // steep at the rim, flat inside
		if (uText1 > 0.001) {
			vec2 tuv = px / uRes;
			vec2 o = vec2(2.5) / uRes;
			float tx = texture2D(uText, tuv + vec2(o.x, 0.0)).r - texture2D(uText, tuv - vec2(o.x, 0.0)).r;
			float ty = texture2D(uText, tuv + vec2(0.0, o.y)).r - texture2D(uText, tuv - vec2(0.0, o.y)).r;
			slope += vec2(tx, ty) * 9.0 * uText1;
		}
		vec2 off = slope * 70.0;
		float disp = 0.06 + uDisp * 0.22;
		// frost: a few taps around the bent point
		vec3 gl = sceneRGB(px + off, off + vec2(0.0001), disp) * 0.4;
		gl += sceneRGB(px + off + vec2(7.0, 3.0), off, disp) * 0.15;
		gl += sceneRGB(px + off + vec2(-6.0, 5.0), off, disp) * 0.15;
		gl += sceneRGB(px + off + vec2(4.0, -7.0), off, disp) * 0.15;
		gl += sceneRGB(px + off + vec2(-5.0, -5.0), off, disp) * 0.15;
		gl = gl * (1.04 + uBright) + 0.035;
		// light on the rim (from the top left) and along the letters
		float rim = smoothstep(4.0, 0.0, abs(d + 2.0));
		vec2 n = length(g) > 0.0 ? normalize(g) : vec2(0.0);
		float lit = 0.35 + 0.65 * max(0.0, dot(-n, normalize(vec2(-0.6, -0.8))));
		gl += rim * lit * 0.55;
		float inner = smoothstep(10.0, 2.0, abs(d + 9.0)) * 0.08;
		gl += inner;
		if (uText1 > 0.001) {
			float t = texture2D(uText, px / uRes).g; // the sharp letters
			gl = mix(gl, gl * 0.55 + 0.45, t * uText1 * 0.85);
		}
		float inside = smoothstep(1.0, -1.0, d);
		// its shadow on the image
		float sh = smoothstep(46.0, 0.0, d) * (1.0 - inside) * 0.22;
		col *= 1.0 - sh * uGlass;
		col = mix(col, gl, inside * uGlass);
	}
	gl_FragColor = vec4(col, 1.0);
}
`;

export async function createGlass(canvas, { images, text }) {
	canvas.width = 1920;
	canvas.height = 1080;
	const gl = canvas.getContext('webgl', { preserveDrawingBuffer: true, antialias: false, premultipliedAlpha: false });
	const sh = (type, src) => {
		const s = gl.createShader(type);
		gl.shaderSource(s, src);
		gl.compileShader(s);
		if (!gl.getShaderParameter(s, gl.COMPILE_STATUS)) throw new Error(gl.getShaderInfoLog(s));
		return s;
	};
	const prog = gl.createProgram();
	gl.attachShader(prog, sh(gl.VERTEX_SHADER, VERT));
	gl.attachShader(prog, sh(gl.FRAGMENT_SHADER, FRAG));
	gl.linkProgram(prog);
	if (!gl.getProgramParameter(prog, gl.LINK_STATUS)) throw new Error(gl.getProgramInfoLog(prog));
	gl.useProgram(prog);
	const buf = gl.createBuffer();
	gl.bindBuffer(gl.ARRAY_BUFFER, buf);
	gl.bufferData(gl.ARRAY_BUFFER, new Float32Array([-1, -1, 3, -1, -1, 3]), gl.STATIC_DRAW);
	const loc = gl.getAttribLocation(prog, 'p');
	gl.enableVertexAttribArray(loc);
	gl.vertexAttribPointer(loc, 2, gl.FLOAT, false, 0, 0);
	const U = (n) => gl.getUniformLocation(prog, n);

	const tex = (src) => {
		const t = gl.createTexture();
		gl.bindTexture(gl.TEXTURE_2D, t);
		gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, gl.RGBA, gl.UNSIGNED_BYTE, src);
		gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.LINEAR);
		gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.LINEAR);
		gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
		gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);
		return t;
	};
	const load = (url) => new Promise((ok, no) => {
		const i = new Image();
		i.onload = () => ok(i);
		i.onerror = () => no(new Error('glass: no ' + url));
		i.src = url;
	});
	const T = {};
	for (const [k, url] of Object.entries(images)) T[k] = tex(await load(url));
	// the letters: red = blurred (height), green = sharp (light)
	const tc = document.createElement('canvas');
	tc.width = 1920;
	tc.height = 1080;
	const c2 = tc.getContext('2d');
	c2.fillStyle = '#000';
	c2.fillRect(0, 0, 1920, 1080);
	c2.textAlign = 'center';
	c2.textBaseline = 'middle';
	c2.font = text.font;
	c2.globalCompositeOperation = 'lighter';
	c2.filter = `blur(${text.blur}px)`;
	c2.fillStyle = '#f00';
	c2.fillText(text.word, text.x, text.y);
	c2.filter = 'none';
	c2.fillStyle = '#0f0';
	c2.fillText(text.word, text.x, text.y);
	T.__text = tex(tc);

	const bind = (unit, name, t) => {
		gl.activeTexture(gl.TEXTURE0 + unit);
		gl.bindTexture(gl.TEXTURE_2D, t);
		gl.uniform1i(U(name), unit);
	};
	gl.viewport(0, 0, 1920, 1080);

	return {
		draw(s) {
			bind(0, 'uA', T[s.a]);
			bind(1, 'uB', T[s.b ?? s.a]);
			bind(2, 'uText', T.__text);
			gl.uniform2f(U('uRes'), 1920, 1080);
			gl.uniform1f(U('uT'), s.t ?? 0);
			gl.uniform1f(U('uWipe'), s.wipe ?? -1);
			const wd = s.wipeDir ?? [1, 0];
			gl.uniform2f(U('uWipeDir'), wd[0], wd[1]);
			const sk = s.shock ?? [0, 0, 0, 0];
			gl.uniform4f(U('uShock'), sk[0], sk[1], sk[2], sk[3]);
			const balls = s.balls ?? [];
			for (let i = 0; i < 4; i++) {
				const b = balls[i];
				gl.uniform4f(U('uBall' + i), b ? b[0] : 0, b ? b[1] : 0, b ? b[2] : 0, b && b[2] > 0.5 ? 1 : 0);
			}
			const r = s.rect ?? [960, 540, 0, 0];
			gl.uniform4f(U('uRect'), r[0], r[1], r[2], r[3]);
			gl.uniform1f(U('uRectR'), s.rectR ?? 80);
			gl.uniform1f(U('uRectOn'), s.rectOn ?? 0);
			gl.uniform1f(U('uGlass'), s.glass ?? 0);
			gl.uniform1f(U('uDisp'), s.disp ?? 0);
			gl.uniform1f(U('uText1'), s.text ?? 0);
			gl.uniform1f(U('uWob'), s.wob ?? 0);
			gl.uniform1f(U('uBright'), s.bright ?? 0);
			gl.uniform1f(U('uKA'), s.kA ?? 1);
			gl.uniform1f(U('uKB'), s.kB ?? 1);
			const pa = s.panA ?? [0, 0], pb = s.panB ?? [0, 0];
			gl.uniform2f(U('uPanA'), pa[0], pa[1]);
			gl.uniform2f(U('uPanB'), pb[0], pb[1]);
			gl.drawArrays(gl.TRIANGLES, 0, 3);
		},
	};
}
