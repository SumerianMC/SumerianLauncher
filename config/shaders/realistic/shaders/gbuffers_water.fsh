#version 120

uniform sampler2D texture;
uniform sampler2D lightmap;
uniform sampler2D shadowtex0;
uniform float frameTimeCounter;

varying vec2 texcoord;
varying vec2 lmcoord;
varying vec4 glcolor;
varying vec3 normal;
varying float isWater;
varying vec4 shadowPos;

/* DRAWBUFFERS:0123 */

float getShadow(vec4 sPos) {
    vec3 proj = sPos.xyz / sPos.w * 0.5 + 0.5;
    if (proj.x < 0.0 || proj.x > 1.0 || proj.y < 0.0 || proj.y > 1.0) return 1.0;
    float spread = 1.0 / 2048.0;
    float shadow = 0.0;
    for (int x = -2; x <= 2; x++) {
        for (int y = -2; y <= 2; y++) {
            float d = texture2D(shadowtex0, proj.xy + vec2(x, y) * spread).r;
            shadow += step(proj.z - 0.0003, d);
        }
    }
    return shadow / 25.0;
}

void main() {
    vec4 albedo = texture2D(texture, texcoord) * glcolor;
    vec4 light  = texture2D(lightmap, lmcoord);

    if (isWater > 0.5) {
        // PBR-style water: Schlick fresnel
        vec3 viewDir = vec3(0.0, 0.0, -1.0);
        float cosTheta = max(dot(normalize(normal), -viewDir), 0.0);
        float F0 = 0.04;
        float fresnel = F0 + (1.0 - F0) * pow(1.0 - cosTheta, 5.0);

        vec3 deepColor    = vec3(0.01, 0.08, 0.28);
        vec3 shallowColor = vec3(0.12, 0.42, 0.72);
        albedo.rgb = mix(shallowColor, deepColor, fresnel);
        albedo.a   = mix(0.4, 0.95, fresnel);

        // Specular highlight
        vec3 lightDir = normalize(vec3(0.6, 0.8, 0.4));
        vec3 halfVec  = normalize(lightDir - viewDir);
        float spec    = pow(max(dot(normal, halfVec), 0.0), 128.0);
        float shadow  = getShadow(shadowPos);
        albedo.rgb   += vec3(spec * 0.9) * shadow * light.rgb;
    }

    float NdotL  = max(dot(normal, normalize(vec3(0.6, 0.8, 0.4))), 0.0);
    float shadow = getShadow(shadowPos);
    albedo.rgb  *= light.rgb * mix(0.3, 1.0, NdotL * shadow);

    float brightness = dot(albedo.rgb, vec3(0.299, 0.587, 0.114));

    gl_FragData[0] = albedo;
    gl_FragData[1] = vec4(normal * 0.5 + 0.5, 1.0);
    gl_FragData[2] = vec4(lmcoord, 0.0, 1.0);
    gl_FragData[3] = vec4(max(brightness - 0.8, 0.0));
}
