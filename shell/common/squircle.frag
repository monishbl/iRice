#version 440
// A filled squircle (continuous corners, the same shape as the glass:
// shell/glass/glass.frag), with an optional border inside its edge. Used by
// common/Squircle.qml. Output is premultiplied.

layout(location = 0) in vec2 qt_TexCoord0;
layout(location = 0) out vec4 fragColor;

layout(std140, binding = 0) uniform buf {
    mat4 qt_Matrix;
    float qt_Opacity;
    vec2 size;
    float radius;
    float borderWidth;
    vec4 fillColor;
    vec4 borderColor;
};

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
    float cov = clamp(0.5 - d, 0.0, 1.0);
    // border: the band within borderWidth of the edge
    float inner = clamp(0.5 - (d + borderWidth), 0.0, 1.0);
    vec4 fill = vec4(fillColor.rgb * fillColor.a, fillColor.a);
    vec4 edge = vec4(borderColor.rgb * borderColor.a, borderColor.a);
    vec4 c = borderWidth > 0.0 ? mix(edge, fill, inner) : fill;
    fragColor = c * cov * qt_Opacity;
}
