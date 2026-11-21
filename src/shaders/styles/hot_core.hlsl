
float4 style(float2 uv, float g)
{
    return float4(lerp(c0.rgb, float3(1, 1, 1), saturate(g * g * 1.5)) * g * 1.3, 0);
}