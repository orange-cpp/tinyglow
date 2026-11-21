
float4 style(float2 uv, float g)
{
    float h = flames(uv, g, 1.0, 0.07);
    float near = max(glow_at(uv + float2(0.0, 0.07 * SIZE)), glow_at(uv + float2(0.0, 0.12 * SIZE)));
    float spark = embers(uv, saturate(near * 2.0) * (1.0 - saturate(h)));
    float s = smoke(uv, g, 0.09) * (1.0 - saturate(h * 2.0));
    return float4(heat(h) * 1.3 + tint(0.4) * spark * 1.6 + 0.03 * s, s * 0.5);
}