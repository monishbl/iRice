#version 300 es
// iRice anti-flashbang: softens very bright regions (white pages at night)
// while leaving mid-tones and colours alone.
precision mediump float;
in vec2 v_texcoord;
uniform sampler2D tex;
out vec4 fragColor;

void main() {
    vec4 c = texture(tex, v_texcoord);
    float l = dot(c.rgb, vec3(0.2126, 0.7152, 0.0722));
    // Above 0.7 luminance, compress towards 0.78
    float k = smoothstep(0.7, 1.0, l);
    float target = mix(l, 0.7 + (l - 0.7) * 0.27, k);
    vec3 rgb = l > 0.0 ? c.rgb * (target / l) : c.rgb;
    fragColor = vec4(rgb, c.a);
}
