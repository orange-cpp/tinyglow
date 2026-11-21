
float4 style(float2 uv, float g)
{
    float h = flames(uv, g, 0.55, 0.1);
    float3 o = c0.rgb * 0.7 * saturate(h * 2.4) + tint(0.5) * 0.35 * saturate(h * 2.4 - 0.8)
             + 0.3 * saturate(h * 2.4 - 1.6);
    return float4(o * 1.2, 0);
}