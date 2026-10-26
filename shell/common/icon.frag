#version 440
// App icon tiles and Tahoe icon styles, in one pass. Output is premultiplied.
//
// tile 0: the icon as drawn.  tile 1: a continuous-corner squircle tile:
//   square icons (they already have a background) are zoomed slightly and
//   clipped, with their own background filling the gaps; transparent logos
//   are scaled down onto a tile of tileColor.
// mode 0 default, 1 dark, 2 clear, 3 tinted. Clear and tinted turn the
//   artwork into a monochrome mask (its difference from the tile colour) and
//   draw it as frosted/tinted glass with a specular rim, like Tahoe.

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
    vec4 tint;        // accent (straight alpha)
    vec4 tileColor;   // tile behind transparent logos
    vec4 bgColor;     // detected background of square icons
};
layout(binding = 1) uniform sampler2D source;

vec4 unpremul(vec4 c) {
    return c.a > 0.0 ? vec4(c.rgb / c.a, c.a) : vec4(0.0);
}
float luma(vec3 c) {
    return dot(c, vec3(0.2126, 0.7152, 0.0722));
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
        rgb = mix(vec3(1.0), vec3(lum), 0.35);
        a *= 0.45 + 0.5 * lum;
    } else {
        rgb = tint.rgb * (0.35 + 0.85 * lum);
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

    // Artwork and the tile under it (straight alpha)
    vec3 base;
    vec4 art;
    if (square > 0.5) {
        art = unpremul(texture(source, (uv - 0.5) * 0.9 + 0.5)); // zoom past its own corners
        base = bgColor.rgb;
    } else {
        vec2 suv = (uv - 0.5) / logoScale + 0.5;
        bool inside = all(greaterThanEqual(suv, vec2(0.0))) && all(lessThanEqual(suv, vec2(1.0)));
        art = inside ? unpremul(texture(source, suv)) : vec4(0.0);
        base = tileColor.rgb;
    }
    vec3 rgb = mix(base, art.rgb, art.a);

    // Glyph mask: how much a pixel differs from the tile, as Tahoe's
    // monochrome layer (black and white artwork)
    float glyph = clamp(art.a * smoothstep(0.06, 0.32, distance(art.rgb, base)) + (square > 0.5 ? 0.0 : art.a * 0.6), 0.0, 1.0);
    float lumArt = luma(art.rgb);
    float rimShade = 0.5 + 0.5 * clamp((-p.x - p.y) * 0.7, -1.0, 1.0); // lit from the top left
    float rim = smoothstep(0.86, 0.995, d) * rimShade;

    if (mode > 0.5 && mode < 1.5) {
        // Dark: deep tile, artwork kept and lifted
        vec3 tileDark = mix(vec3(0.21, 0.21, 0.23), vec3(0.13, 0.13, 0.15), uv.y);
        float keep = clamp(glyph * 1.4, 0.0, 1.0);
        rgb = mix(tileDark, min(art.rgb * 1.1 + 0.04, vec3(1.0)), keep);
        rgb += rim * 0.10;
    } else if (mode > 1.5 && mode < 2.5) {
        // Clear: frosted glass tile, glyph etched as brighter frost
        float a = 0.32 + 0.08 * (1.0 - uv.y);
        vec3 tileC = darkTheme > 0.5 ? vec3(0.62, 0.64, 0.68) : vec3(0.97, 0.97, 0.99);
        vec3 glyphC = vec3(1.0);
        float g = glyph * (0.75 + 0.25 * lumArt);
        rgb = mix(tileC, glyphC, g * 0.9);
        float alpha = mix(a, 0.92, g) + rim * 0.35;
        rgb += rim * 0.35;
        fragColor = vec4(clamp(rgb, 0.0, 1.0) * alpha, alpha) * shape * qt_Opacity;
        return;
    } else if (mode > 2.5) {
        // Tinted: dark tinted glass tile, glyph in the tint, lighter where the art is light
        vec3 tileT = darkTheme > 0.5 ? mix(vec3(0.07), tint.rgb, 0.22) : mix(vec3(1.0), tint.rgb, 0.18);
        vec3 glyphT = darkTheme > 0.5 ? mix(tint.rgb, vec3(1.0), 0.15 + 0.35 * lumArt) : tint.rgb * (0.55 + 0.45 * lumArt);
        rgb = mix(tileT, glyphT, glyph);
        rgb += rim * 0.22;
    } else {
        rgb += rim * 0.06; // default: a hint of edge light
    }
    fragColor = vec4(clamp(rgb, 0.0, 1.0), 1.0) * shape * qt_Opacity;
}
