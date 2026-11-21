
float4 style(float2 uv, float g)
{
    float w = 0.5 + 0.5 * sin(g * 18.0 / SIZE - TIME * 6.0);
    return float4(c0.rgb * g * (0.25 + 0.9 * w), 0);
}