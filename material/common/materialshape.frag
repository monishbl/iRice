#version 440
// One of Material 3 Expressive's shapes (cookies, sunny, clover, pill, …),
// or a morph between two of them, filled with a colour or a picture (pictureTex), with an
// optional border inside its edge. Used by common/MaterialShape.qml (the
// shapes are in MaterialShapes.js). Each shape is a distance-like field in
// unit coordinates (the item's shorter side spans -1..1); a morph mixes the
// two fields. Output is premultiplied.
//   kind 0 "star": radius 1 - a·((1 - cos nθ)/2)^b (cookies, sunny, clover…)
//   kind 1 rounded regular polygon: n sides, corner rounding a
//   kind 2 rounded box: half-height b (width 1), corner rounding a
//   kind 3 ellipse: half-height a (width 1)

layout(location = 0) in vec2 qt_TexCoord0;
layout(location = 0) out vec4 fragColor;

layout(std140, binding = 0) uniform buf {
    mat4 qt_Matrix;
    float qt_Opacity;
    vec2 size;
    vec4 shapeA;   // kind, n, a, b
    vec4 shapeB;
    vec2 turnA;    // each shape's own rotation (radians), and its scale
    vec2 turnB;
    float morph;   // 0 shapeA … 1 shapeB
    float turn;    // rotation of the whole (radians)
    float borderWidth;
    float picture; // 1: fill with `source`
    vec4 fillColor;
    vec4 borderColor;
};
layout(binding = 1) uniform sampler2D pictureTex;

const float PI = 3.14159265;

float sdPolygon(vec2 p, float n, float r) {
    // a regular polygon of circumradius r, one vertex pointing up
    float an = PI / n;
    float a = atan(p.x, p.y);
    float bn = mod(a + an, 2.0 * an) - an;
    vec2 q = length(p) * vec2(cos(bn), abs(sin(bn)));
    q -= r * vec2(cos(an), sin(an));
    q.y += clamp(-q.y, 0.0, r * sin(an));
    return length(q) * sign(q.x);
}

float field(vec2 p, vec4 s, vec2 t) {
    float c = cos(t.x), sn = sin(t.x);
    p = mat2(c, -sn, sn, c) * p / max(t.y, 1e-3);
    float kind = s.x, n = s.y, a = s.z, b = s.w;
    float d;
    if (kind < 0.5) {
        float th = atan(p.y, p.x);
        float r = 1.0 - a * pow(max(0.0, 0.5 - 0.5 * cos(n * th)), b);
        d = (length(p) - r) * r; // (scaled, so it stays a distance near the edge)
    } else if (kind < 1.5) {
        d = sdPolygon(p, n, 1.0 - a) - a;
    } else if (kind < 2.5) {
        vec2 half_ = vec2(1.0, b);
        float r = min(a, b);
        vec2 q = abs(p) - half_ + r;
        d = length(max(q, 0.0)) + min(max(q.x, q.y), 0.0) - r;
    } else {
        vec2 ab = vec2(1.0, a);
        float k0 = length(p / ab);
        float k1 = length(p / (ab * ab));
        d = k0 * (k0 - 1.0) / max(k1, 1e-4);
    }
    return d * t.y;
}

void main() {
    float unit = 0.5 * min(size.x, size.y);
    vec2 p = (qt_TexCoord0 * size - 0.5 * size) / unit;
    float c = cos(turn), s = sin(turn);
    vec2 q = mat2(c, -s, s, c) * p;
    float d = mix(field(q, shapeA, turnA), field(q, shapeB, turnB), morph) * unit; // px
    float cov = clamp(0.5 - d, 0.0, 1.0);
    float inner = clamp(0.5 - (d + borderWidth), 0.0, 1.0);
    vec4 fill = picture > 0.5 ? texture(pictureTex, qt_TexCoord0) : fillColor;
    vec4 col = borderWidth > 0.0 ? mix(borderColor, fill, inner) : fill;
    fragColor = col * cov * qt_Opacity;
}
