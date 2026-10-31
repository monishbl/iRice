#version 440
// Cuts a layer to a squircle (the glass's continuous corners: the same
// shape as squircle.frag), worked out here from the size and radius. In
// place of MultiEffect's mask, which stopped drawing the island's black and
// album art after the island shrank and grew again. Output is premultiplied.

layout(location = 0) in vec2 qt_TexCoord0;
layout(location = 0) out vec4 fragColor;

layout(std140, binding = 0) uniform buf {
    mat4 qt_Matrix;
    float qt_Opacity;
    vec2 size;
    float radius;
};
layout(binding = 1) uniform sampler2D source;

float sdSquircle(vec2 p, vec2 b, float r) {
    float hm = min(b.x, b.y);
    float k = smoothstep(0.65, 1.0, clamp(r / max(hm, 1e-3), 0.0, 1.0));
    float n = mix(4.0, 2.0, k);
    float re = min(r * mix(1.28, 1.0, k), hm);
    vec2 q = abs(p) - b + re;
    vec2 m = max(q, 0.0);
    float l = pow(pow(m.x, n) + pow(m.y, n), 1.0 / n);
    return l + min(max(q.x, q.y), 0.0) - re;
}

void main() {
    vec2 half_ = size * 0.5;
    vec2 p = qt_TexCoord0 * size - half_;
    float d = sdSquircle(p, half_, min(radius, min(half_.x, half_.y)));
    fragColor = texture(source, qt_TexCoord0) * (clamp(0.5 - d, 0.0, 1.0) * qt_Opacity);
}
