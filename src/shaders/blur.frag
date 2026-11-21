#version 430 core

layout(binding = 0) uniform sampler2D input_texture;
uniform vec4 parameters;
in vec2 uv;
layout(location = 0) out vec4 color;

const float offsets[6] = float[6](1.3366, 3.4295, 5.4264, 7.4359, 9.4436, 11.4401);
const float weights[6] = float[6](0.2185, 0.0821, 0.0461, 0.0262, 0.0162, 0.0102);

void main()
{
    float coverage = texture(input_texture, uv).r * 0.2013;
    for (int i = 0; i < 6; ++i) {
        vec2 delta = parameters.xy * offsets[i];
        coverage += (texture(input_texture, uv + delta).r
                   + texture(input_texture, uv - delta).r) * weights[i];
    }
    color = vec4(vec3(coverage * parameters.z), 1.0);
}
