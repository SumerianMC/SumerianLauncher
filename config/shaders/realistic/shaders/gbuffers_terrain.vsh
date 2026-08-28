#version 120

attribute vec4 mc_Entity;

varying vec2 texcoord;
varying vec2 lmcoord;
varying vec4 glcolor;
varying vec3 normal;
varying vec4 shadowPos;

uniform mat4 shadowProjection;
uniform mat4 shadowModelView;
uniform mat4 gbufferModelViewInverse;

void main() {
    texcoord = (gl_TextureMatrix[0] * gl_MultiTexCoord0).xy;
    lmcoord  = (gl_TextureMatrix[1] * gl_MultiTexCoord1).xy;
    glcolor  = gl_Color;
    normal   = normalize(gl_NormalMatrix * gl_Normal);

    vec4 worldPos = gbufferModelViewInverse * (gl_ModelViewMatrix * gl_Vertex);
    shadowPos     = shadowProjection * (shadowModelView * worldPos);

    gl_Position = ftransform();
}
