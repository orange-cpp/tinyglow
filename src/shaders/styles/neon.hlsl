
float4 style(float2 uv, float g)
{
    return float4(c0.rgb * pow(g, 0.6) * 1.2 + g * g * g * 0.7, 0);
}