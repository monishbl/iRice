#version 440
// Fades a layer out toward its edges (to transparent, not to a colour), so
// a picture melts into whatever is behind it. fade = how far in each edge's
// fall-off reaches, as a fraction of the size: left, top, right, bottom.
// Output is premultiplied.

layout(location = 0) in vec2 qt_TexCoord0;
layout(location = 0) out vec4 fragColor;

layout(std140, binding = 0) uniform buf {
    mat4 qt_Matrix;
    float qt_Opacity;
    vec4 fade;
};
layout(binding = 1) uniform sampler2D source;

void main() {
    vec2 uv = qt_TexCoord0;
    float a = smoothstep(0.0, max(fade.x, 1e-4), uv.x)
            * smoothstep(0.0, max(fade.y, 1e-4), uv.y)
            * smoothstep(0.0, max(fade.z, 1e-4), 1.0 - uv.x)
            * smoothstep(0.0, max(fade.w, 1e-4), 1.0 - uv.y);
    fragColor = texture(source, uv) * (a * qt_Opacity);
}
