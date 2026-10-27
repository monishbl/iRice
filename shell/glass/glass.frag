#version 440
// Liquid glass for a rounded rectangle, in one pass.
//   mode 0 (liquid):  refracts a pre-blurred wallpaper. The rim bends outward, so
//                     what lies just outside the shape folds into its edge (the
//                     lensing Tahoe shows), with slight chromatic dispersion.
//   mode 1 (frosted): translucent tint only; the compositor blurs what's behind.
//   mode 2 (solid):   opaque, for Reduce Transparency.
//   mode 3 (plugin):  shadow only; the irice-glass Hyprland plugin draws the glass.
// All modes add a specular rim and a soft drop shadow. Output is premultiplied.

layout(location = 0) in vec2 qt_TexCoord0;
layout(location = 0) out vec4 fragColor;

layout(std140, binding = 0) uniform buf {
    mat4 qt_Matrix;
    float qt_Opacity;
    vec2 itemSize;     // item size in px, shadow margin included
    vec4 shapeRect;    // glass rect inside the item: x, y, w, h (px)
    vec4 screenRect;   // the item's rect on screen (px)
    vec2 screenSize;   // screen size (px)
    float radius;
    float bevel;       // width of the refracting rim (px)
    float refraction;  // displacement at the very edge (px)
    float chroma;      // dispersion, 0..1
    float rim;         // specular rim strength
    float mode;
    float saturation;
    float shadow;      // drop shadow opacity
    vec4 tintColor;    // straight alpha
    vec4 solidColor;
    float gloss;       // 3D lighting amount: 1 glass, ~0 frost (even hairline)
};
layout(binding = 1) uniform sampler2D backdrop;

float sdRoundBox(vec2 p, vec2 b, float r) {
    // Continuous (squircle) corners, like iOS and macOS: the corner is a
    // superellipse (n = 4) reaching a little further along the sides than a
    // circular arc of the same radius, so the curve eases in instead of
    // starting abruptly. Near-pills blend back to circular ends (n -> 2), so
    // capsules and circles stay round. Same function in shell/glass/glass.frag
    // and the irice-glass plugin, so both draw the same shape.
    float hm = min(b.x, b.y);
    float k = smoothstep(0.65, 1.0, clamp(r / max(hm, 1e-3), 0.0, 1.0));
    float n = mix(4.0, 2.0, k);
    float re = min(r * mix(1.28, 1.0, k), hm);
    vec2 q = abs(p) - b + re;
    vec2 m = max(q, 0.0);
    float l = pow(pow(m.x, n) + pow(m.y, n), 1.0 / n);
    return l + min(max(q.x, q.y), 0.0) - re;
}

// Outward normal of that shape, continuous around the corners (the
// superellipse's gradient there, the sides' normals elsewhere).
vec2 roundBoxNormal(vec2 p, vec2 b, float r) {
    float hm = min(b.x, b.y);
    float k = smoothstep(0.65, 1.0, clamp(r / max(hm, 1e-3), 0.0, 1.0));
    float n = mix(4.0, 2.0, k);
    float re = min(r * mix(1.28, 1.0, k), hm);
    vec2 q = abs(p) - b + re;
    vec2 g = (q.x > 0.0 && q.y > 0.0) ? normalize(pow(q, vec2(n - 1.0)) + 1e-6) : (q.x > q.y ? vec2(1.0, 0.0) : vec2(0.0, 1.0));
    return g * vec2(p.x < 0.0 ? -1.0 : 1.0, p.y < 0.0 ? -1.0 : 1.0);
}

void main() {
    vec2 px = qt_TexCoord0 * itemSize;
    vec2 halfSize = shapeRect.zw * 0.5;
    vec2 p = px - (shapeRect.xy + halfSize);
    float r = min(radius, min(halfSize.x, halfSize.y));
    float d = sdRoundBox(p, halfSize, r);

    // Soft shadow, slightly below the shape. Fades out fully inside the item's
    // margin (shapeRect.x) so it never gets clipped into a hard edge.
    float sd = sdRoundBox(p - vec2(0.0, shapeRect.x * 0.2), halfSize, r);
    float fall = 1.0 - smoothstep(-shapeRect.x * 0.5, shapeRect.x * 0.8, sd);
    float shadowA = shadow * fall * fall * fall;

    float coverage = clamp(0.5 - d, 0.0, 1.0);
    // Mode 3: the compositor plugin draws the glass; only the shadow is ours
    if (mode > 2.5) {
        fragColor = vec4(0.0, 0.0, 0.0, shadowA * (1.0 - coverage)) * qt_Opacity;
        return;
    }
    if (coverage <= 0.0) {
        fragColor = vec4(0.0, 0.0, 0.0, shadowA) * qt_Opacity;
        return;
    }

    float depth = -d;
    vec2 n = roundBoxNormal(p, halfSize, r);
    float t = clamp(1.0 - depth / max(bevel, 1.0), 0.0, 1.0); // 1 at the edge
    float lens = t * t * (3.0 - 2.0 * t);

    vec3 col;
    float alpha;
    if (mode < 0.5) {
        vec2 at = screenRect.xy + px;
        vec2 off = n * refraction * lens;
        vec2 inv = 1.0 / screenSize;
        col = vec3(texture(backdrop, (at + off * (1.0 + chroma)) * inv).r,
                   texture(backdrop, (at + off) * inv).g,
                   texture(backdrop, (at + off * (1.0 - chroma)) * inv).b);
        float l = dot(col, vec3(0.2126, 0.7152, 0.0722));
        col = mix(vec3(l), col, saturation);
        col = mix(col, tintColor.rgb, tintColor.a);
        alpha = 1.0;
    } else if (mode < 1.5) {
        col = tintColor.rgb;
        alpha = tintColor.a;
    } else {
        col = solidColor.rgb;
        alpha = 1.0;
    }

    // Light from the top left (same model as the irice-glass plugin): a sharp
    // specular where it hits, a fainter glint on the opposite edge, a sheen
    // across the top and an inner shadow along the bottom.
    vec2 L = normalize(vec2(-0.55, -0.83));
    float facing = dot(n, L);
    float line = 1.0 - smoothstep(0.0, 1.4, depth);
    float band = 1.0 - smoothstep(0.0, max(bevel * 0.22, 2.0), depth);
    float key = pow(max(facing, 0.0), 10.0);
    float back = pow(max(-facing, 0.0), 8.0) * 0.25;
    float spec = rim * (line * mix(0.45, 0.2 + 0.6 * max(facing, 0.0), gloss) + gloss * (band * (key * 0.55 + back) + 0.08 * lens * max(facing, 0.0)));
    float yIn = clamp((px.y - shapeRect.y) / max(shapeRect.w, 1.0), 0.0, 1.0);
    spec += rim * gloss * 0.04 * pow(1.0 - yIn, 4.0);
    col += spec;
    col *= 1.0 - rim * gloss * 0.12 * band * max(n.y, 0.0) * (1.0 - key);
    alpha = clamp(alpha + spec, 0.0, 1.0);

    vec4 glass = vec4(col * alpha, alpha) * coverage;
    fragColor = (glass + vec4(0.0, 0.0, 0.0, shadowA) * (1.0 - glass.a)) * qt_Opacity;
}
