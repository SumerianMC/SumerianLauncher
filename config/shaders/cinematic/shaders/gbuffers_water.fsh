#version 120

uniform sampler2D texture;
uniform sampler2D lightmap;
uniform float frameTimeCounter;

varying vec2 texcoord;
varying vec2 lmcoord;
varying vec4 glcolor;
varying vec3 normal;
varying float isWater;

/* DRAWBUFFERS:0123 */

void main() {
    vec4 albedo = texture2D(texture, texcoord) * glcolor;
    vec4 light  = texture2D(lightmap, lmcoord);

    if (isWater > 0.5) {
        float fresnel = pow(1.0 - abs(dot(normalize(normal), vec3(0.0, 1.0, 0.0))), 3.0);
        vec3 deepColor     = vec3(0.02, 0.12, 0.35);
        vec3 shallowColor  = vec3(0.18, 0.52, 0.82);
        albedo.rgb = mix(shallowColor, deepColor, fresnel);
        albedo.a   = mix(0.45, 0.92, fresnel);

        // Specular
        vec3 lightDir = normalize(vec3(0.5, 0.9, 0.3));
        float spec = pow(max(dot(reflect(-lightDir, normal), vec3(0.0, 0.0, -1.0)), 0.0), 64.0);
        albedo.rgb += vec3(spec * 0.7) * light.rgb;
    }

    float brightness = dot(albedo.rgb * light.rgb, vec3(0.299, 0.587, 0.114));

    gl_FragData[0] = albedo * light;
    gl_FragData[1] = vec4(normal * 0.5 + 0.5, 1.0);
    gl_FragData[2] = vec4(lmcoord, 0.0, 1.0);
    gl_FragData[3] = vec4(max(brightness - 0.7, 0.0));
}
