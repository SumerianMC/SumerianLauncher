#version 120

attribute vec4 mc_Entity;

uniform mat4 gbufferModelView;
uniform mat4 gbufferModelViewInverse;
uniform mat4 shadowProjection;
uniform mat4 shadowModelView;
uniform float frameTimeCounter;

varying vec2 texcoord;
varying vec2 lmcoord;
varying vec4 glcolor;
varying vec3 normal;
varying float isWater;
varying vec4 shadowPos;

void main() {
    texcoord = (gl_TextureMatrix[0] * gl_MultiTexCoord0).xy;
    lmcoord  = (gl_TextureMatrix[1] * gl_MultiTexCoord1).xy;
    glcolor  = gl_Color;
    isWater  = float(mc_Entity.x == 8.0 || mc_Entity.x == 9.0);

    vec4 pos = gl_ModelViewMatrix * gl_Vertex;

    if (isWater > 0.5) {
        vec4 world = gbufferModelViewInverse * pos;
        float wave = sin(world.x * 1.8 + frameTimeCounter * 2.2) * 0.05
                   + sin(world.z * 2.1 + frameTimeCounter * 1.9) * 0.04
                   + cos(world.x * 0.9 + world.z * 1.2 + frameTimeCounter * 1.5) * 0.03;
        world.y += wave;
        pos = gbufferModelView * world;

        // Recalculate normal from wave gradient
        vec3 dx = vec3(1.0, cos(world.x * 1.8 + frameTimeCounter * 2.2) * 0.09, 0.0);
        vec3 dz = vec3(0.0, cos(world.z * 2.1 + frameTimeCounter * 1.9) * 0.084, 1.0);
        normal = normalize(cross(dz, dx));
    } else {
        normal = normalize(gl_NormalMatrix * gl_Normal);
    }

    vec4 worldPos = gbufferModelViewInverse * pos;
    shadowPos     = shadowProjection * (shadowModelView * worldPos);
    gl_Position   = gl_ProjectionMatrix * pos;
}
