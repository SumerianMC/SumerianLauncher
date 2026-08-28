#version 120

uniform sampler2D gcolor;

varying vec2 texcoord;

// Chromatic aberration + gamma
void main() {
    vec2 dir = (texcoord - 0.5) * 0.005;
    float r = texture2D(gcolor, texcoord + dir).r;
    float g = texture2D(gcolor, texcoord      ).g;
    float b = texture2D(gcolor, texcoord - dir).b;
    vec3 color = pow(max(vec3(r, g, b), vec3(0.0)), vec3(1.0 / 2.2));
    gl_FragColor = vec4(color, 1.0);
}
