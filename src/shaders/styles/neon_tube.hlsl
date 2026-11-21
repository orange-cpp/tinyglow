
float4 style(float2 uv, float g)
{
    STYLE_BRANCH if (far_away(uv, g))
        return float4(0, 0, 0, 0);
    float inside = body_at(uv);
    float tube1 = saturate(grown(uv, 2.0 * SIZE) - inside);
    float tube2 = saturate(grown(uv, 7.0 * SIZE) - grown(uv, 5.0 * SIZE));
    float hum = 0.9 + 0.1 * sin(TIME * 40.0);
    float3 hot = lerp(c0.rgb, float3(1, 1, 1), 0.55);
    return float4((hot * (tube1 + tube2 * 0.8) * 1.4 + c0.rgb * g * 0.35) * hum, 0);
}