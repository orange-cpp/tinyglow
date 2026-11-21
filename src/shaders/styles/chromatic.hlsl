
float4 style(float2 uv, float g)
{
    float2 o = TEXEL * 6.0 * SIZE;
    float3 split = hue_turn(1.0 / 3.0) * glow_at(uv + o) + c0.rgb * g + hue_turn(2.0 / 3.0) * glow_at(uv - o);
    return float4(split * 0.7, 0);
}