#version 440
// Display connect / disconnect (modules/osd/DisplayConnect.qml). One screen,
// one shared edge (the side facing the other display), three looks:
//   style 0 extend: a wide, soft, motion-blurred wipe from the shared edge
//   style 1 liquid: a glass pill pops up on the neighbour, stretches toward
//                   the edge and buds off a droplet (a gooey neck that thins
//                   and snaps); the droplet arrives on the new screen with a
//                   springy overshoot and swells to fill it, its edge
//                   feathering more the larger it grows
//   style 2 warp:   the wallpaper grows out of the edge through a lens
// role 0: the screen that just arrived, dark until the effect fills it with
//         the wallpaper (source)
// role 1: a screen next to it (or next to one that left): light only, no
//         wallpaper, so its windows stay visible; reverse = 1 moves inward
//         from the edge (a display left), 0 outward toward it (one arrived)
// progress runs 0 .. 1. Output is premultiplied.

layout(location = 0) in vec2 qt_TexCoord0;
layout(location = 0) out vec4 fragColor;

layout(std140, binding = 0) uniform buf {
    mat4 qt_Matrix;
    float qt_Opacity;
    float progress;
    float style;
    float role;
    float edge;    // the shared edge: 0 left, 1 top, 2 right, 3 bottom
    float along;   // where along that edge the other display is (0 .. 1)
    float reverse;
    vec2 size;     // px, for round drops
    vec4 tint;     // glass light (accent-ish), premultiplied later
};
layout(binding = 1) uniform sampler2D source;

// d: 0 at the shared edge .. 1 across; t: along the edge
void frame(vec2 uv, out float d, out float t) {
    int e = int(edge + 0.5);
    if (e == 0) { d = uv.x; t = uv.y; }
    else if (e == 2) { d = 1.0 - uv.x; t = uv.y; }
    else if (e == 1) { d = uv.y; t = uv.x; }
    else { d = 1.0 - uv.y; t = uv.x; }
}
// back from (d, t) to uv
vec2 unframe(float d, float t) {
    int e = int(edge + 0.5);
    if (e == 0) return vec2(d, t);
    if (e == 2) return vec2(1.0 - d, t);
    if (e == 1) return vec2(t, d);
    return vec2(t, 1.0 - d);
}
// length of one unit of d and t in px
vec2 scale() {
    int e = int(edge + 0.5);
    return (e == 0 || e == 2) ? size : size.yx;
}

float hash(float n) { return fract(sin(n * 91.345) * 47453.21); }

// smooth union (polynomial smin): a gooey neck between shapes closer than k
float smin(float a, float b, float k) {
    float h = max(k - abs(a - b), 0.0) / max(k, 1e-4);
    return min(a, b) - h * h * k * 0.25;
}
// a damped spring 0 → 1, overshooting ~12% (framer-motion bounce ≈ 0.16)
float spring(float x) {
    x = clamp(x, 0.0, 1.0);
    return 1.0 - exp(-5.0 * x) * cos(7.5 * x);
}
// capsule along x from x0 to x1 (centres of its round ends), radius r
float capsule(vec2 q, float x0, float x1, float r) {
    return length(vec2(q.x - clamp(q.x, x0, x1), q.y)) - r;
}
// The liquid's drop size: the round button of the pill (px)
float dropR() { return min(size.x, size.y) * 0.034; }

// The neighbour's glass pill and the drop it buds off. q runs 0 → 1 the way
// the motion goes (time reversed for a display that left: the drop comes
// back and merges in). In px, x = distance from the shared edge, y = along
// it from `along`. Returns the shape's signed distance; fade in `alpha`.
float pillAndDrop(vec2 P, float q, out float alpha) {
    float rr = dropR();
    float sx = scale().x;
    // pop in from below (spring), then stretch toward the edge
    float pop = spring(q / 0.16);
    float r = rr * pop;
    float yoff = (1.0 - min(pop, 1.0)) * rr * 2.5;
    float farX = 0.24 * sx + rr;
    float lmax = min(11.0 * rr, 0.2 * sx);
    float L = 2.0 * rr + spring((q - 0.12) / 0.30) * (lmax - 2.0 * rr);
    float nearX = farX - L;
    // the split: the near end becomes the drop; the pill pulls back a little
    float c = clamp((q - 0.38) / 0.34, 0.0, 1.0);
    float retreat = smoothstep(0.0, 0.45, c) * 2.3 * rr;
    float tr = max(c - 0.25, 0.0) / 0.75;
    float travel = tr * tr * (nearX + 2.0 * rr + 8.0);
    // and shrinks away at the end
    float e = clamp((q - 0.72) / 0.28, 0.0, 1.0);
    float shrink = 1.0 - e * e;
    float mid = (nearX + retreat + farX) * 0.5;
    float x0 = mix(mid, nearX + retreat + r, shrink);
    float x1 = mix(mid, farX - r, shrink);
    vec2 Q = vec2(P.x, P.y - yoff);
    float pill = capsule(Q, min(x0, x1), max(x0, x1), r * mix(0.3, 1.0, shrink));
    // the drop, stretched a little along its travel (liquid)
    float dx = nearX + rr - travel;
    float stretch = 1.0 + 0.35 * sin(tr * 3.14159);
    // (no drop before the split)
    float drop = c > 0.0 ? length(vec2((Q.x - dx) / stretch, Q.y)) - r : 1e4;
    float goo = 1.4 * rr * (1.0 - smoothstep(0.7, 1.0, c));
    float sd = smin(pill, drop, goo);
    // drawn into the edge as it leaves
    sd = smin(sd, Q.x + rr * 0.3, rr * 1.2 * smoothstep(0.55, 0.85, c) * (1.0 - smoothstep(0.9, 1.0, c)));
    alpha = smoothstep(0.0, 0.04, q) * (1.0 - smoothstep(0.85, 1.0, e));
    return sd;
}

// Liquid glass, lit (no picture behind it): premultiplied
vec4 glass(float sd, vec3 light, float rr) {
    float body = smoothstep(1.0, -1.0, sd);
    float rim = exp(-pow((sd + 1.6) / 1.8, 2.0));
    float inner = exp(-pow((sd + rr * 0.45) / (rr * 0.35), 2.0)) * body; // the bevel's glow
    float shadow = exp(-max(sd, 0.0) / 10.0) * (1.0 - body) * 0.22;
    float a = body * 0.14 + rim * 0.6 + inner * 0.12;
    vec3 rgb = light * a;
    return vec4(rgb, min(1.0, a + shadow * (1.0 - a)));
}

void main() {
    vec2 uv = qt_TexCoord0;
    float d, t;
    frame(uv, d, t);
    float p = clamp(progress, 0.0, 1.0);
    int st = int(style + 0.5);
    vec3 light = tint.rgb;
    vec4 col = vec4(0.0);

    if (role < 0.5) {
        // ── The screen that arrived ─────────────────────────────────────
        vec4 dark = vec4(0.0, 0.0, 0.0, 1.0);
        if (st == 0) {
            // A wide, soft wipe from the shared edge: the picture slides in
            // behind a broad feathered front, motion-blurred until it settles
            float e = 1.0 - pow(1.0 - p, 3.0); // ease-out
            float F = 0.6; // the feather: most of a screen wide
            float front = mix(0.3, 1.0 + F, e); // the soft front is already coming in
            float inside = 1.0 - smoothstep(front - F, front, d);
            inside *= inside * (3.0 - 2.0 * inside);
            float slide = (1.0 - e) * 0.18; // the picture travels in with the front
            float R = 0.16 * (1.0 - e);
            vec3 acc = vec3(0.0);
            // (the picture squeezed toward the far side, so sampling never
            // runs off it and smears its last column)
            float dd = mix(slide, 1.0, d);
            for (int i = 0; i < 16; i++) {
                float o = (float(i) / 15.0 - 0.5) * R;
                acc += texture(source, unframe(clamp(dd + o * (1.0 - dd) * 2.0, 0.0, 1.0), t)).rgb;
            }
            col = mix(dark, vec4(acc / 16.0, 1.0), inside);
        } else if (st == 1) {
            // The drop arrives through the edge (springy), then swells to fill
            vec2 s = scale();
            float rr = dropR();
            vec2 P = vec2(d * s.x, (t - along) * s.y);
            // arriving and swelling overlap, so the drop never stops
            float a = clamp(p / 0.4, 0.0, 1.0);
            float g = clamp((p - 0.16) / 0.84, 0.0, 1.0);
            float ge = g * g * (3.0 - 2.0 * g);
            float x1 = 0.12 * s.x;
            vec2 c0 = vec2(-rr + (x1 + rr) * spring(a), 0.0);
            // swelling, it drifts to the middle of the screen
            vec2 mid = vec2(0.5 * s.x, (0.5 - along) * s.y);
            vec2 C = mix(c0, mid, ge);
            // the feather grows with the drop: crisp while small, melting
            // into the whole screen as it fills it
            float feather = 1.5 + ge * ge * min(s.x, s.y) * 0.45;
            float Rmax = length(s) * 0.5 + length(mid - c0) * 0.6 + feather + 24.0;
            float R = rr + (Rmax - rr) * pow(ge, 1.3);
            float sd = length(P - C) - R;
            // still joined to the edge at first: a neck that lets go
            sd = smin(sd, P.x + rr * 0.3, rr * 1.3 * (1.0 - smoothstep(0.15, 0.5, a)));
            float inside = smoothstep(feather, -feather, sd);
            vec3 pic = texture(source, uv).rgb;
            col = mix(dark, vec4(pic, 1.0), inside);
            // a thin glass rim while it's a drop, gone as it grows
            col.rgb += light * exp(-pow((sd + 1.6) / 1.8, 2.0)) * 0.5 * (1.0 - smoothstep(0.0, 0.35, ge));
        } else {
            // warp: the picture grows out of the edge, squeezed at first
            float k = mix(5.0, 1.0, smoothstep(0.0, 1.0, p));
            float sd = pow(clamp(d / max(p * 1.15, 1e-3), 0.0, 1.0), 1.0 / k) * min(p * 1.15, 1.0);
            float pinch = mix(2.2, 1.0, smoothstep(0.0, 0.9, p));
            float stt = along + (t - along) * mix(pinch, 1.0, smoothstep(0.0, 1.0, d / max(p, 1e-3)));
            float inside = (1.0 - smoothstep(p * 1.15 - 0.05, p * 1.15, d)) * step(0.0, stt) * step(stt, 1.0);
            vec4 pic = vec4(texture(source, unframe(clamp(sd, 0.0, 1.0), clamp(stt, 0.0, 1.0))).rgb, 1.0);
            float ring = exp(-pow((d - p * 1.15) / 0.02, 2.0));
            col = mix(dark, pic, inside);
            col.rgb += light * ring * 0.45;
        }
    } else {
        // ── A screen beside it: light at the shared edge, nothing solid ──
        // q: 0 → 1 the way the light travels (outward to the edge, or in from it)
        float q = reverse > 0.5 ? p : 1.0 - p;
        float fade = sin(p * 3.14159);
        vec2 s = scale();
        float aspect = s.y / s.x;
        float a = 0.0;
        if (st == 0) {
            // a faint, wide glow at the shared edge, nothing more
            float near = exp(-pow((t - along) * 1.2, 2.0));
            float al = exp(-d / 0.14) * 0.16 * fade * mix(0.5, 1.0, near);
            fragColor = vec4(light * al, al) * qt_Opacity;
            return;
        } else if (st == 1) {
            vec2 P = vec2(d * s.x, (t - along) * s.y);
            float alpha;
            float sd = pillAndDrop(P, reverse > 0.5 ? 1.0 - p : p, alpha);
            fragColor = glass(sd, light, dropR()) * alpha * qt_Opacity;
            return;
        } else {
            // a lens ring running to (or from) the edge
            float pos = q * 1.0;
            float ring = exp(-pow((d - pos) / 0.025, 2.0));
            float near = exp(-pow((t - along) * 1.4, 2.0));
            a = ring * 0.4 * mix(0.35, 1.0, near) * fade + exp(-d / 0.03) * 0.35 * fade;
        }
        col = vec4(light * a, a);
    }
    fragColor = col * qt_Opacity;
}
