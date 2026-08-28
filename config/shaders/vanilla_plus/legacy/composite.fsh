#version 120

// Vanilla Plus — Legacy post-processing
// Compatible with GLSL Shaders Mod (Minecraft 1.0-1.6.4)
// Applies: FXAA, contrast lift, saturation boost, vignette

uniform sampler2D gcolor;
uniform float viewWidth;
uniform float viewHeight;

varying vec2 texcoord;

vec3 fxaa(sampler2D tex, vec2 uv) {
    vec2 texel = 1.0 / vec2(viewWidth, viewHeight);
    vec3 rgbNW = texture2D(tex, uv + vec2(-1.0, -1.0) * texel).rgb;
    vec3 rgbNE = texture2D(tex, uv + vec2( 1.0, -1.0) * texel).rgb;
    vec3 rgbSW = texture2D(tex, uv + vec2(-1.0,  1.0) * texel).rgb;
    vec3 rgbSE = texture2D(tex, uv + vec2( 1.0,  1.0) * texel).rgb;
    vec3 rgbM  = texture2D(tex, uv).rgb;
    vec3 luma  = vec3(0.299, 0.587, 0.114);
    float lumaNW = dot(rgbNW, luma); float lumaNE = dot(rgbNE, luma);
    float lumaSW = dot(rgbSW, luma); float lumaSE = dot(rgbSE, luma);
    float lumaM  = dot(rgbM, luma);
    float lumaMin = min(lumaM, min(min(lumaNW, lumaNE), min(lumaSW, lumaSE)));
    float lumaMax = max(lumaM, max(max(lumaNW, lumaNE), max(lumaSW, lumaSE)));
    vec2 dir = vec2(-((lumaNW+lumaNE)-(lumaSW+lumaSE)), (lumaNW+lumaSW)-(lumaNE+lumaSE));
    float rcpMin = 1.0 / (min(abs(dir.x), abs(dir.y)) + max((lumaNW+lumaNE+lumaSW+lumaSE)*0.03125, 0.0078125));
    dir = clamp(dir * rcpMin, -8.0, 8.0) * texel;
    vec3 rgbA = 0.5 * (texture2D(tex, uv + dir*(1.0/3.0-0.5)).rgb + texture2D(tex, uv + dir*(2.0/3.0-0.5)).rgb);
    vec3 rgbB = rgbA*0.5 + 0.25*(texture2D(tex, uv+dir*-0.5).rgb + texture2D(tex, uv+dir*0.5).rgb);
    float lumaB = dot(rgbB, luma);
    return (lumaB < lumaMin || lumaB > lumaMax) ? rgbA : rgbB;
}

void main() {
    vec3 color = fxaa(gcolor, texcoord);
    color = pow(color, vec3(0.95));
    float grey = dot(color, vec3(0.299, 0.587, 0.114));
    color = mix(vec3(grey), color, 1.12);
    float dist = distance(texcoord, vec2(0.5));
    color *= smoothstep(0.75, 0.3, dist);
    gl_FragColor = vec4(color, 1.0);
}
