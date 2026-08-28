#version 120

// Realistic — Legacy post-processing
// ACES filmic tone mapping, saturation, vignette, 4-tap MSAA softening
// Compatible with GLSL Shaders Mod (Minecraft 1.0-1.6.4)

uniform sampler2D gcolor;
uniform float viewWidth;
uniform float viewHeight;

varying vec2 texcoord;

// ACES filmic tone mapping
vec3 aces(vec3 x) {
    float a = 2.51, b = 0.03, c = 2.43, d = 0.59, e = 0.14;
    return clamp((x * (a * x + b)) / (x * (c * x + d) + e), 0.0, 1.0);
}

// 4-tap MSAA box softening
vec3 msaa(sampler2D tex, vec2 uv) {
    vec2 t = 0.5 / vec2(viewWidth, viewHeight);
    return (texture2D(tex, uv + vec2( t.x,  t.y)).rgb
          + texture2D(tex, uv + vec2(-t.x,  t.y)).rgb
          + texture2D(tex, uv + vec2( t.x, -t.y)).rgb
          + texture2D(tex, uv + vec2(-t.x, -t.y)).rgb) * 0.25;
}

void main() {
    vec3 color = msaa(gcolor, texcoord);

    // ACES tone mapping
    color = aces(color * 1.1);

    // Saturation
    float grey = dot(color, vec3(0.299, 0.587, 0.114));
    color = mix(vec3(grey), color, 1.15);

    // Vignette
    float dist = distance(texcoord, vec2(0.5)) * 1.3;
    color *= smoothstep(0.9, 0.25, dist);

    gl_FragColor = vec4(color, 1.0);
}
