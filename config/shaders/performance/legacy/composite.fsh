#version 120

uniform sampler2D gcolor;

varying vec2 texcoord;

void main() {
    gl_FragColor = texture2D(gcolor, texcoord);
}
