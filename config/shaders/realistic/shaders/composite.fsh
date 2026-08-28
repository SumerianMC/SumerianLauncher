#version 120

uniform sampler2D colortex0;
uniform float viewWidth;
uniform float viewHeight;

varying vec2 texcoord;

/* DRAWBUFFERS:0 */

vec3 bloom(vec2 uv) {
    vec2 t = 2.0 / vec2(viewWidth, viewHeight);
    vec3 c = vec3(0.0);
    c += max(texture2D(colortex0, uv + vec2( 1.5,  0.5) * t).rgb - 0.7, 0.0);
    c += max(texture2D(colortex0, uv + vec2(-1.5,  0.5) * t).rgb - 0.7, 0.0);
    c += max(texture2D(colortex0, uv + vec2( 0.5,  1.5) * t).rgb - 0.7, 0.0);
    c += max(texture2D(colortex0, uv + vec2( 0.5, -1.5) * t).rgb - 0.7, 0.0);
    c += max(texture2D(colortex0, uv + vec2( 3.0,  1.0) * t).rgb - 0.7, 0.0) * 0.5;
    c += max(texture2D(colortex0, uv + vec2(-3.0,  1.0) * t).rgb - 0.7, 0.0) * 0.5;
    c += max(texture2D(colortex0, uv + vec2( 1.0,  3.0) * t).rgb - 0.7, 0.0) * 0.5;
    c += max(texture2D(colortex0, uv + vec2( 1.0, -3.0) * t).rgb - 0.7, 0.0) * 0.5;
    return c / 6.0;
}

vec3 aces(vec3 x) {
    return clamp((x*(2.51*x+0.03))/(x*(2.43*x+0.59)+0.14), 0.0, 1.0);
}

void main() {
    vec3 color = texture2D(colortex0, texcoord).rgb;

    color += bloom(texcoord) * 0.25;
    color = aces(color * 1.1);

    float grey = dot(color, vec3(0.299, 0.587, 0.114));
    color = mix(vec3(grey), color, 1.15);

    gl_FragData[0] = vec4(color, 1.0);
}
