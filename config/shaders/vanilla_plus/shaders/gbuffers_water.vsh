#version 120

attribute vec4 mc_Entity;
attribute vec4 mc_midTexCoord;
attribute vec4 at_tangent;

varying vec2 texcoord;
varying vec2 lmcoord;
varying vec4 glcolor;
varying vec3 normal;
varying vec3 worldPos;
varying float isWater;

uniform mat4 gbufferModelView;
uniform mat4 gbufferModelViewInverse;
uniform float frameTimeCounter;

void main() {
    texcoord  = (gl_TextureMatrix[0] * gl_MultiTexCoord0).xy;
    lmcoord   = (gl_TextureMatrix[1] * gl_MultiTexCoord1).xy;
    glcolor   = gl_Color;
    normal    = normalize(gl_NormalMatrix * gl_Normal);
    isWater   = float(mc_Entity.x == 8.0 || mc_Entity.x == 9.0);

    vec4 pos = gl_ModelViewMatrix * gl_Vertex;

    // Waving water surface
    if (isWater > 0.5) {
        vec4 worldVertex = gbufferModelViewInverse * pos;
        worldVertex.y += sin(worldVertex.x * 1.5 + frameTimeCounter * 2.0) * 0.04
                       + sin(worldVertex.z * 1.5 + frameTimeCounter * 1.7) * 0.04;
        pos = gbufferModelView * worldVertex;
    }

    worldPos    = (gbufferModelViewInverse * pos).xyz;
    gl_Position = gl_ProjectionMatrix * pos;
}
