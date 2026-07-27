#version 120

// Cinematic — Legacy post-processing
// Bloom approximation via large-radius blur, warm/cool color grade, film grain

uniform sampler2D gcolor;
uniform float viewWidth;
uniform float viewHeight;
uniform float frameTimeCounter;

varying vec2 texcoord;

// 9-tap box blur used as cheap bloom source
vec3 blurSample(sampler2D tex, vec2 uv, float radius) {
    vec2 t = radius / vec2(viewWidth, viewHeight);
    vec3 c = vec3(0.0);
    c += texture2D(tex, uv + vec2(-1.0, -1.0) * t).rgb;
    c += texture2D(tex, uv + vec2( 0.0, -1.0) * t).rgb;
    c += texture2D(tex, uv + vec2( 1.0, -1.0) * t).rgb;
    c += texture2D(tex, uv + vec2(-1.0,  0.0) * t).rgb;
    c += texture2D(tex, uv).rgb * 2.0;
    c += texture2D(tex, uv + vec2( 1.0,  0.0) * t).rgb;
    c += texture2D(tex, uv + vec2(-1.0,  1.0) * t).rgb;
    c += texture2D(tex, uv + vec2( 0.0,  1.0) * t).rgb;
    c += texture2D(tex, uv + vec2( 1.0,  1.0) * t).rgb;
    return c / 10.0;
}

void main() {
    vec3 color = texture2D(gcolor, texcoord).rgb;

    // Bloom: extract bright areas and add blurred version
    vec3 blurred = blurSample(gcolor, texcoord, 3.0);
    float brightness = dot(blurred, vec3(0.299, 0.587, 0.114));
    color += max(blurred - 0.7, vec3(0.0)) * 0.4;

    // Warm shadows, cool highlights color grade
    float lum = dot(color, vec3(0.299, 0.587, 0.114));
    color *= mix(vec3(1.08, 0.96, 0.84), vec3(0.88, 0.94, 1.12), lum);

    // S-curve contrast
    color = color / (color + vec3(0.4)) * 1.4;

    // Saturation
    float grey = dot(color, vec3(0.299, 0.587, 0.114));
    color = mix(vec3(grey), color, 1.2);

    // Film grain
    float grain = fract(sin(dot(texcoord + fract(frameTimeCounter), vec2(127.1, 311.7))) * 43758.5453) * 0.04 - 0.02;
    color += grain;

    // Vignette
    float dist = distance(texcoord, vec2(0.5)) * 1.5;
    color *= smoothstep(1.0, 0.2, dist);

    gl_FragColor = vec4(max(color, vec3(0.0)), 1.0);
}
