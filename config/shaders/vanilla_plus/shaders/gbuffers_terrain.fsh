#version 120

uniform sampler2D texture;
uniform sampler2D lightmap;
uniform sampler2D shadowtex0;
uniform sampler2D shadowtex1;
uniform sampler2D shadowcolor0;
uniform mat4 shadowProjection;
uniform mat4 shadowModelView;
uniform mat4 gbufferModelViewInverse;

varying vec2 texcoord;
varying vec2 lmcoord;
varying vec4 glcolor;
varying vec3 normal;

/* DRAWBUFFERS:0123 */

// PCF soft shadow sampling
float getShadow(vec4 shadowPos) {
    shadowPos.xyz = shadowPos.xyz * 0.5 + 0.5;
    float shadow = 0.0;
    float spread = 1.0 / 1024.0;
    for (int x = -1; x <= 1; x++) {
        for (int y = -1; y <= 1; y++) {
            vec2 offset = vec2(float(x), float(y)) * spread;
            float depth = texture2D(shadowtex0, shadowPos.xy + offset).r;
            shadow += step(shadowPos.z - 0.0005, depth);
        }
    }
    return shadow / 9.0;
}

void main() {
    vec4 albedo = texture2D(texture, texcoord) * glcolor;
    if (albedo.a < 0.1) discard;

    vec4 light = texture2D(lightmap, lmcoord);

    // Directional shading from normal
    float NdotL = max(dot(normal, normalize(vec3(0.6, 0.8, 0.4))), 0.0);
    float shade = mix(0.4, 1.0, NdotL);

    albedo.rgb *= light.rgb * shade;

    gl_FragData[0] = albedo;
    gl_FragData[1] = vec4(normal * 0.5 + 0.5, 1.0);
    gl_FragData[2] = vec4(lmcoord, 0.0, 1.0);
    gl_FragData[3] = vec4(0.0, 0.0, 0.0, 1.0);
}
