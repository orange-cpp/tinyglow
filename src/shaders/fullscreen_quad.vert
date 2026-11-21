#version 430 core

out vec2 uv;

void main()
{
    uv = vec2(gl_VertexID & 1, (gl_VertexID >> 1) & 1);
    gl_Position = vec4(uv * 2.0 - 1.0, 0.0, 1.0);
}
