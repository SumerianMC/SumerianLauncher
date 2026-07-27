#version 120

uniform sampler2D gcolor;

varying vec2 texcoord;

void main() {
    vec3 color = texture2D(gcolor, texcoord).rgb;
    color = pow(max(color, vec3(0.0)), vec3(1.0 / 2.2));
    gl_FragColor = vec4(color, 1.0);
}
