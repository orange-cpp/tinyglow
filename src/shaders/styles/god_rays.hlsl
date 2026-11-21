
float4 style(float2 uv, float g)
{
    float2 step_uv = (uv - float2(0.5, 0.45)) * 0.035 * SIZE;
    float2 p = uv;
    float rays = 0.0;
    float w = 1.0;
    STYLE_UNROLL for (int i = 0; i < 8; i++)
    {
        p -= step_uv;
        rays += glow_at(p) * w;
        w *= 0.85;
    }
    return float4(lerp(c0.rgb, float3(1, 1, 1), 0.3) * (rays * 0.22 + g * 0.4), 0);
}