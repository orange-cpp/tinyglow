
float4 style(float2 uv, float g)
{
    STYLE_BRANCH if (far_away(uv, g))
        return float4(0, 0, 0, 0);
    float inside = body_at(uv);
    float rim = inside * saturate(openness(uv, 3.0 * SIZE) * 2.5);
    return float4(lerp(c0.rgb, float3(1, 1, 1), 0.35) * rim * 1.4 + c0.rgb * (1.0 - inside) * g * 0.6, 0);
}