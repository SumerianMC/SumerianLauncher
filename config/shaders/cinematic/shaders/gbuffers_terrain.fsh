#version 120

uniform sampler2D texture;
uniform sampler2D lightmap;

varying vec2 texcoord;
varying vec2 lmcoord;
varying vec4 glcolor;
varying vec3 normal;

/* DRAWBUFFERS:0123 */

void main() {
    vec4 albedo = texture2D(texture, texcoord) * glcolor;
    if (albedo.a < 0.1) discard;

    vec4 light  = texture2D(lightmap, lmcoord);
    float NdotL = max(dot(normal, normalize(vec3(0.5, 0.9, 0.3))), 0.0);
    float shade = mix(0.35, 1.0, NdotL);

    albedo.rgb *= light.rgb * shade;

    // Store emissive brightness for bloom extraction
    float brightness = dot(albedo.rgb, vec3(0.299, 0.587, 0.114));

    gl_FragData[0] = albedo;
    gl_FragData[1] = vec4(normal * 0.5 + 0.5, 1.0);
    gl_FragData[2] = vec4(lmcoord, 0.0, 1.0);
    gl_FragData[3] = vec4(max(brightness - 0.75, 0.0));  // bloom threshold
}
