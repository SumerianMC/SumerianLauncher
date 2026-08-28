#version 120

attribute vec4 mc_Entity;

uniform mat4 gbufferModelView;
uniform mat4 gbufferModelViewInverse;
uniform float frameTimeCounter;

varying vec2 texcoord;
varying vec2 lmcoord;
varying vec4 glcolor;
varying vec3 normal;
varying float isWater;

void main() {
    texcoord = (gl_TextureMatrix[0] * gl_MultiTexCoord0).xy;
    lmcoord  = (gl_TextureMatrix[1] * gl_MultiTexCoord1).xy;
    glcolor  = gl_Color;
    normal   = normalize(gl_NormalMatrix * gl_Normal);
    isWater  = float(mc_Entity.x == 8.0 || mc_Entity.x == 9.0);

    vec4 pos = gl_ModelViewMatrix * gl_Vertex;

    if (isWater > 0.5) {
        vec4 world = gbufferModelViewInverse * pos;
        world.y += sin(world.x * 2.0 + frameTimeCounter * 2.5) * 0.06
                 + cos(world.z * 1.8 + frameTimeCounter * 2.0) * 0.06;
        pos = gbufferModelView * world;
    }

    gl_Position = gl_ProjectionMatrix * pos;
}
