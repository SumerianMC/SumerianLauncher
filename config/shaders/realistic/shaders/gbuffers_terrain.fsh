#version 120

uniform sampler2D texture;
uniform sampler2D lightmap;
uniform sampler2D shadowtex0;
uniform sampler2D shadowtex1;

varying vec2 texcoord;
varying vec2 lmcoord;
varying vec4 glcolor;
varying vec3 normal;
varying vec4 shadowPos;

/* DRAWBUFFERS:0123 */

// 16-tap Poisson PCF soft shadows
float getShadow(vec4 sPos) {
    vec3 proj = sPos.xyz / sPos.w * 0.5 + 0.5;
    if (proj.x < 0.0 || proj.x > 1.0 || proj.y < 0.0 || proj.y > 1.0) return 1.0;

    float shadow = 0.0;
    float spread = 1.0 / 2048.0;
    vec2 poisson[16];
    poisson[ 0] = vec2(-0.94, -0.94); poisson[ 1] = vec2(-0.09, -0.94);
    poisson[ 2] = vec2( 0.34, -0.85); poisson[ 3] = vec2(-0.91, -0.41);
    poisson[ 4] = vec2(-0.81,  0.19); poisson[ 5] = vec2(-0.38, -0.40);
    poisson[ 6] = vec2( 0.97, -0.15); poisson[ 7] = vec2( 0.44, -0.43);
    poisson[ 8] = vec2( 0.53,  0.05); poisson[ 9] = vec2(-0.26,  0.18);
    poisson[10] = vec2( 0.79,  0.68); poisson[11] = vec2(-0.24,  0.68);
    poisson[12] = vec2( 0.14,  0.93); poisson[13] = vec2(-0.81,  0.68);
    poisson[14] = vec2( 0.14,  0.45); poisson[15] = vec2(-0.50,  0.93);

    for (int i = 0; i < 16; i++) {
        float depth = texture2D(shadowtex0, proj.xy + poisson[i] * spread).r;
        shadow += step(proj.z - 0.0003, depth);
    }
    return shadow / 16.0;
}

void main() {
    vec4 albedo = texture2D(texture, texcoord) * glcolor;
    if (albedo.a < 0.1) discard;

    vec4 light  = texture2D(lightmap, lmcoord);
    float NdotL = max(dot(normal, normalize(vec3(0.6, 0.8, 0.4))), 0.0);
    float shadow = getShadow(shadowPos);
    float shade  = mix(0.3, 1.0, NdotL * shadow);

    albedo.rgb *= light.rgb * shade;

    float brightness = dot(albedo.rgb, vec3(0.299, 0.587, 0.114));

    gl_FragData[0] = albedo;
    gl_FragData[1] = vec4(normal * 0.5 + 0.5, 1.0);
    gl_FragData[2] = vec4(lmcoord, 0.0, 1.0);
    gl_FragData[3] = vec4(max(brightness - 0.8, 0.0));
}
