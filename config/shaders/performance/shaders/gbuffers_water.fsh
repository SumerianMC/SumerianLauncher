#version 120

uniform sampler2D texture;
uniform sampler2D lightmap;

varying vec2 texcoord;
varying vec2 lmcoord;
varying vec4 glcolor;

/* DRAWBUFFERS:0 */

void main() {
    vec4 albedo = texture2D(texture, texcoord) * glcolor;
    albedo.rgb *= texture2D(lightmap, lmcoord).rgb;
    gl_FragData[0] = albedo;
}
