
float4 style(float2 uv, float g)
{
    float a = smoothstep(0.08, 0.12, g) * (1.0 - smoothstep(0.2, 0.24, g));
    float b = smoothstep(0.4, 0.45, g) * (1.0 - smoothstep(0.55, 0.6, g));
    return float4(c0.rgb * (a + b) * 1.2, 0);
}