
float4 style(float2 uv, float g)
{
    float s = smoke(uv, g, 0.06);
    return float4(c0.rgb * 0.12 * s + c0.rgb * g * 0.15, s * 0.75);
}