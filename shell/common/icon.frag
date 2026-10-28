#version 440
// App icon tiles and Tahoe icon styles, in one pass. Output is premultiplied.
//
// tile 0: the icon as drawn.  tile 1: a continuous-corner squircle tile:
//   square icons (they already have a background) are zoomed slightly and
//   clipped, with their own background filling the gaps; transparent logos
//   are scaled down onto a tile of tileColor.
// mode 0 default, 1 dark, 2 clear, 3 tinted. Clear and tinted draw a
//   translucent glass tile with a specular rim, like Tahoe, and the logo on
//   it as one tone (light on dark glass, dark on light glass, or the accent):
//   the logo is what differs from the background it sits on (a tile, or a
//   disc or rounded plate inside a transparent icon, as IconAnalyzer measured
//   it: a top-to-bottom gradient, bgColor to bgColor2, and how much it varies
//   itself, bgFloor), or for bare logos their opaque part. Its inner detail
//   stays as shades around the logo's own typical tone (logoMed, spread
//   logoLo..logoHi), so every logo reads alike whatever its colours;
//   `contrast` (Settings → Appearance → Icon contrast) sets how far those
//   shades reach and how solid the logo is.

layout(location = 0) in vec2 qt_TexCoord0;
layout(location = 0) out vec4 fragColor;

layout(std140, binding = 0) uniform buf {
    mat4 qt_Matrix;
    float qt_Opacity;
    float mode;
    float tile;
    float square;     // 1: the icon already has a full background
    float logoScale;  // transparent logos: size on the tile
    float darkTheme;
    float contrast;   // 0..1, how strongly clear/tinted logos stand out
    float plate;      // 1: a transparent icon carrying its own disc/plate
    float logoLo;     // the logo's tones (IconAnalyzer): 10th percentile,
    float logoHi;     //   90th,
    float logoMed;    //   and median luminance
    float logoPol;    // +1 lighter reads brighter, -1 darker does (on a tile: away from it)
    float bgFloor;    // how much the background itself varies (gradients)
    float bgEdge;     // how far the logo typically stands from the background
    vec4 tint;        // accent (straight alpha)
    vec4 tileColor;   // tile behind transparent logos
    vec4 bgColor;     // the background behind the logo, at the top…
    vec4 bgColor2;    // …and at the bottom
};
layout(binding = 1) uniform sampler2D source;

vec4 unpremul(vec4 c) {
    return c.a > 0.0 ? vec4(c.rgb / c.a, c.a) : vec4(0.0);
}
float luma(vec3 c) {
    return dot(c, vec3(0.2126, 0.7152, 0.0722));
}

// A logo pixel's shade, 0..1: the logo's typical tone is bright (0.75);
// moving toward the background's tone (or darker, for bare logos) dims it, in
// steps of the logo's own spread, so dark and washed-out logos read alike
float shade(float lum) {
    float t = (lum - logoMed) / max(logoHi - logoLo, 0.12) * logoPol;
    return clamp(0.75 + 0.5 * t, 0.0, 1.0);
}
// The logo's colour in clear mode: light on dark glass, dark on light glass;
// its shades reach further with more contrast
vec3 clearGlyph(float n) {
    float r = mix(0.2, 0.65, contrast);
    return darkTheme > 0.5 ? vec3(1.0 - r + r * n) : vec3(0.1 + 0.9 * r * (1.0 - n));
}
// …and in tinted mode, in the accent
vec3 tintGlyph(float n) {
    float r = mix(0.2, 0.65, contrast);
    return darkTheme > 0.5 ? mix(tint.rgb * (1.0 - 0.6 * r), mix(tint.rgb, vec3(1.0), 0.35), n)
                           : mix(mix(tint.rgb, vec3(1.0), 0.45 * r), tint.rgb * 0.55, n);
}

// Original styles for untiled icons
vec4 styleFree(vec4 c) {
    if (c.a <= 0.0 || mode < 0.5)
        return c;
    vec3 rgb = c.rgb / c.a;
    float lum = luma(rgb);
    float sat = max(rgb.r, max(rgb.g, rgb.b)) - min(rgb.r, min(rgb.g, rgb.b));
    float a = c.a;
    if (mode < 1.5) {
        float bg = smoothstep(0.6, 0.85, lum) * (1.0 - smoothstep(0.12, 0.3, sat));
        rgb = mix(min(rgb * 1.12 + 0.03, vec3(1.0)), mix(vec3(0.16, 0.16, 0.18), vec3(0.24, 0.24, 0.27), lum), bg);
    } else if (mode < 2.5) {
        rgb = clearGlyph(shade(lum));
        a *= mix(0.85, 1.0, contrast);
    } else {
        rgb = tintGlyph(shade(lum));
        a *= mix(0.85, 1.0, contrast);
    }
    return vec4(rgb * a, a);
}

void main() {
    vec2 uv = qt_TexCoord0;
    if (tile < 0.5) {
        fragColor = styleFree(texture(source, uv)) * qt_Opacity;
        return;
    }

    // Continuous-corner (superellipse) tile with a 1px-ish antialiased edge
    vec2 p = uv * 2.0 - 1.0;
    const float n = 5.0;
    float d = pow(pow(abs(p.x), n) + pow(abs(p.y), n), 1.0 / n);
    float aa = max(fwidth(d), 1e-3);
    float shape = 1.0 - smoothstep(1.0 - aa * 1.5, 1.0, d);
    if (shape <= 0.0) {
        fragColor = vec4(0.0);
        return;
    }

    // Artwork and the tile under it (straight alpha); vy: where in the icon
    // (top to bottom) this pixel is, for the background's gradient
    vec3 base;
    vec4 art;
    float vy;
    if (square > 0.5) {
        vec2 suv = (uv - 0.5) * 0.9 + 0.5; // zoom past its own corners
        art = unpremul(texture(source, suv));
        base = mix(bgColor.rgb, bgColor2.rgb, 0.5);
        vy = suv.y;
    } else {
        vec2 suv = (uv - 0.5) / logoScale + 0.5;
        bool inside = all(greaterThanEqual(suv, vec2(0.0))) && all(lessThanEqual(suv, vec2(1.0)));
        art = inside ? unpremul(texture(source, suv)) : vec4(0.0);
        base = tileColor.rgb;
        vy = suv.y;
    }
    vec3 rgb = mix(base, art.rgb, art.a);

    // The logo: on a tile or plate, what differs from the background there by
    // more than the background varies itself; a bare logo, its opaque part
    float glyph = art.a;
    if (square > 0.5 || plate > 0.5) {
        vec3 behind = mix(bgColor.rgb, bgColor2.rgb, clamp(vy, 0.0, 1.0));
        glyph *= smoothstep(bgFloor + 0.04, bgFloor + max(0.1, bgEdge * 0.45), distance(art.rgb, behind));
    }
    float lumN = shade(luma(art.rgb));
    float rimShade = 0.5 + 0.5 * clamp((-p.x - p.y) * 0.7, -1.0, 1.0); // lit from the top left
    float rim = smoothstep(0.86, 0.995, d) * rimShade;

    if (mode > 0.5 && mode < 1.5) {
        // Dark: deep tile, artwork kept and lifted
        vec3 tileDark = mix(vec3(0.21, 0.21, 0.23), vec3(0.13, 0.13, 0.15), uv.y);
        float keep = square > 0.5 ? clamp(glyph * 1.4, 0.0, 1.0) : art.a; // (transparent icons keep all their art, plates too)
        rgb = mix(tileDark, min(art.rgb * 1.1 + 0.04, vec3(1.0)), keep);
        rgb += rim * 0.10;
    } else if (mode > 1.5) {
        // Clear and tinted: a translucent glass tile (what's behind shows
        // through) with the logo on it in one tone; tinted washes the tile
        // faintly with the accent and draws the logo in it
        vec3 tileC = darkTheme > 0.5 ? vec3(0.16, 0.17, 0.19) : vec3(0.96, 0.96, 0.98);
        float a = (darkTheme > 0.5 ? 0.42 : 0.5) + 0.08 * (1.0 - uv.y);
        vec3 ink;
        if (mode < 2.5) {
            ink = clearGlyph(lumN);
        } else {
            tileC = mix(tileC, tint.rgb, 0.12);
            ink = tintGlyph(lumN);
        }
        float g = glyph * mix(0.85, 1.0, contrast);
        rgb = mix(tileC, ink, g) + rim * (darkTheme > 0.5 ? 0.3 : 0.12);
        float alpha = min(1.0, mix(a, 1.0, g) + rim * 0.3);
        fragColor = vec4(clamp(rgb, 0.0, 1.0) * alpha, alpha) * shape * qt_Opacity;
        return;
    } else {
        rgb += rim * 0.06; // default: a hint of edge light
    }
    fragColor = vec4(clamp(rgb, 0.0, 1.0), 1.0) * shape * qt_Opacity;
}
