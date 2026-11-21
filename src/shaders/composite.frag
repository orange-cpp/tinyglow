#version 430 core

layout(binding = 0) uniform sampler2D input_texture;
uniform vec4 parameters;
in vec2 uv;
layout(location = 0) out vec4 color;

void main()
{
    float coverage = textureLod(input_texture, uv, 0.0).r;
    color = vec4((parameters.rgb * coverage) * parameters.a, 0.0);
}
