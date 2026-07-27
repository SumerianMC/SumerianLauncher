#version 120

uniform sampler2D texture;
uniform sampler2D lightmap;
uniform float frameTimeCounter;
uniform int isEyeInWater;

varying vec2 texcoord;
varying vec2 lmcoord;
varying vec4 glcolor;
varying vec3 normal;
varying vec3 worldPos;
varying float isWater;

/* DRAWBUFFERS:0123 */

void main() {
    vec4 albedo = texture2D(texture, texcoord) * glcolor;
    vec4 light  = texture2D(lightmap, lmcoord);

    if (isWater > 0.5) {
        // Animated water tint with fresnel-like edge darkening
        float fresnel = 1.0 - abs(dot(normalize(normal), vec3(0.0, 1.0, 0.0)));
        fresnel = clamp(fresnel, 0.0, 1.0);
        vec3 waterColor = mix(vec3(0.15, 0.45, 0.75), vec3(0.05, 0.15, 0.45), fresnel);
        albedo.rgb = mix(albedo.rgb, waterColor, 0.65);
        albedo.a   = mix(0.55, 0.85, fresnel);

        // Specular highlight
        vec3 lightDir = normalize(vec3(0.6, 0.8, 0.4));
        float spec = pow(max(dot(reflect(-lightDir, normal), vec3(0.0, 0.0, -1.0)), 0.0), 32.0);
        albedo.rgb += vec3(spec * 0.4);
    }

    gl_FragData[0] = albedo * light;
    gl_FragData[1] = vec4(normal * 0.5 + 0.5, 1.0);
    gl_FragData[2] = vec4(lmcoord, 0.0, 1.0);
    gl_FragData[3] = vec4(isWater, 0.0, 0.0, 1.0);
}
