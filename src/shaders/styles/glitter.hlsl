
float4 style(float2 uv, float g)
{
    float r = hash(floor(uv / TEXEL / (3.0 * SIZE)));
    float twinkle = pow(0.5 + 0.5 * sin(TIME * 6.0 + r * 40.0), 4.0);
    float star = step(0.965, r) * twinkle;
    return float4(c0.rgb * g * 0.5 + lerp(c0.rgb, float3(1, 1, 1), 0.7) * star * saturate(g * 3.0) * 3.0, 0);
}