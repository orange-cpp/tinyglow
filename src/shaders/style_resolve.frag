#version 430 core

layout(binding = 0) uniform sampler2D styled_color;
uniform vec4 parameters;
layout(location = 0) out vec4 fragment;

void main()
{
    ivec2 pixel = ivec2(gl_FragCoord.xy);
    fragment = texelFetch(styled_color, ivec2(pixel.x, int(parameters.x) - 1 - pixel.y), 0);
}
