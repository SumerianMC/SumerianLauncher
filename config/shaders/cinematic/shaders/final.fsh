#version 120

uniform sampler2D colortex0;
uniform float viewWidth;
uniform float viewHeight;

varying vec2 texcoord;

// Cinematic letterbox vignette
vec3 vignette(vec3 color, vec2 uv) {
    float dist = distance(uv, vec2(0.5)) * 1.4;
    float vig  = smoothstep(0.9, 0.2, dist);
    return color * mix(0.55, 1.0, vig);
}

// Chromatic aberration
vec3 chromaticAberration(sampler2D tex, vec2 uv) {
    vec2 dir    = (uv - 0.5) * 0.004;
    float r = texture2D(tex, uv + dir).r;
    float g = texture2D(tex, uv      ).g;
    float b = texture2D(tex, uv - dir).b;
    return vec3(r, g, b);
}

void main() {
    vec3 color = chromaticAberration(colortex0, texcoord);
    color = vignette(color, texcoord);
    // Film grain
    float grain = fract(sin(dot(texcoord, vec2(127.1, 311.7))) * 43758.5453) * 0.03 - 0.015;
    color += grain;
    color = pow(max(color, vec3(0.0)), vec3(1.0 / 2.2));
    gl_FragColor = vec4(color, 1.0);
}
