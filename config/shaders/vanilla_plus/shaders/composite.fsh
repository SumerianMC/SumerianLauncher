#version 120

uniform sampler2D colortex0;
uniform float viewWidth;
uniform float viewHeight;

varying vec2 texcoord;

/* DRAWBUFFERS:0 */

vec3 fxaa(vec2 uv) {
    vec2 t = 1.0 / vec2(viewWidth, viewHeight);
    vec3 rgbNW = texture2D(colortex0, uv + vec2(-1.0, -1.0) * t).rgb;
    vec3 rgbNE = texture2D(colortex0, uv + vec2( 1.0, -1.0) * t).rgb;
    vec3 rgbSW = texture2D(colortex0, uv + vec2(-1.0,  1.0) * t).rgb;
    vec3 rgbSE = texture2D(colortex0, uv + vec2( 1.0,  1.0) * t).rgb;
    vec3 rgbM  = texture2D(colortex0, uv).rgb;
    vec3 luma  = vec3(0.299, 0.587, 0.114);
    float lumaNW = dot(rgbNW, luma), lumaNE = dot(rgbNE, luma);
    float lumaSW = dot(rgbSW, luma), lumaSE = dot(rgbSE, luma);
    float lumaM  = dot(rgbM,  luma);
    float lumaMin = min(lumaM, min(min(lumaNW, lumaNE), min(lumaSW, lumaSE)));
    float lumaMax = max(lumaM, max(max(lumaNW, lumaNE), max(lumaSW, lumaSE)));
    vec2 dir = vec2(-((lumaNW+lumaNE)-(lumaSW+lumaSE)), (lumaNW+lumaSW)-(lumaNE+lumaSE));
    float rcpMin = 1.0 / (min(abs(dir.x), abs(dir.y)) + max((lumaNW+lumaNE+lumaSW+lumaSE)*0.03125, 0.0078125));
    dir = clamp(dir * rcpMin, -8.0, 8.0) * t;
    vec3 rgbA = 0.5 * (texture2D(colortex0, uv + dir*(1.0/3.0-0.5)).rgb
                     + texture2D(colortex0, uv + dir*(2.0/3.0-0.5)).rgb);
    vec3 rgbB = rgbA*0.5 + 0.25*(texture2D(colortex0, uv+dir*-0.5).rgb
                                + texture2D(colortex0, uv+dir* 0.5).rgb);
    float lumaB = dot(rgbB, luma);
    return (lumaB < lumaMin || lumaB > lumaMax) ? rgbA : rgbB;
}

void main() {
    vec3 color = fxaa(texcoord);
    // Subtle contrast
    color = pow(max(color, vec3(0.0)), vec3(0.95));
    // Saturation boost
    float grey = dot(color, vec3(0.299, 0.587, 0.114));
    color = mix(vec3(grey), color, 1.1);
    gl_FragData[0] = vec4(color, 1.0);
}
