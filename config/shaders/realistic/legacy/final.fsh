#version 120

uniform sampler2D gcolor;
uniform float viewWidth;
uniform float viewHeight;

varying vec2 texcoord;

// 4-tap MSAA + gamma
void main() {
    vec2 t = 0.5 / vec2(viewWidth, viewHeight);
    vec3 color = (texture2D(gcolor, texcoord + vec2( t.x,  t.y)).rgb
                + texture2D(gcolor, texcoord + vec2(-t.x,  t.y)).rgb
                + texture2D(gcolor, texcoord + vec2( t.x, -t.y)).rgb
                + texture2D(gcolor, texcoord + vec2(-t.x, -t.y)).rgb) * 0.25;
    gl_FragColor = vec4(pow(max(color, vec3(0.0)), vec3(1.0 / 2.2)), 1.0);
}
