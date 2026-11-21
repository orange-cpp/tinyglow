
float4 style(float2 uv, float g)
{
    STYLE_BRANCH if (far_away(uv, g))
        return float4(0, 0, 0, 0);
    float inside = body_at(uv);
    float lines = 0.75 + 0.25 * sin(uv.y / TEXEL.y * 1.5 / SIZE);
    float rim = saturate(openness(uv, 2.5 * SIZE) * 2.5);
    float fill = inside * (0.18 + rim * 0.9) * lines;
    float flick = 0.92 + 0.08 * sin(TIME * 30.0);
    return float4(c0.rgb * (fill * flick + (1.0 - inside) * g * 0.35), 0);
}