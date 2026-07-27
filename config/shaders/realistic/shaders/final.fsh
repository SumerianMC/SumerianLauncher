#version 120

uniform sampler2D colortex0;
uniform float viewWidth;
uniform float viewHeight;

varying vec2 texcoord;

// Subtle vignette
vec3 vignette(vec3 color, vec2 uv) {
    float d   = distance(uv, vec2(0.5)) * 1.3;
    float vig = smoothstep(0.85, 0.25, d);
    return color * mix(0.65, 1.0, vig);
}

// MSAA-style edge softening via 4-tap box
vec3 msaa(sampler2D tex, vec2 uv) {
    vec2 t = 0.5 / vec2(viewWidth, viewHeight);
    return (texture2D(tex, uv + vec2( t.x,  t.y)).rgb
          + texture2D(tex, uv + vec2(-t.x,  t.y)).rgb
          + texture2D(tex, uv + vec2( t.x, -t.y)).rgb
          + texture2D(tex, uv + vec2(-t.x, -t.y)).rgb) * 0.25;
}

void main() {
    vec3 color = msaa(colortex0, texcoord);
    color = vignette(color, texcoord);
    color = pow(max(color, vec3(0.0)), vec3(1.0 / 2.2));
    gl_FragColor = vec4(color, 1.0);
}
