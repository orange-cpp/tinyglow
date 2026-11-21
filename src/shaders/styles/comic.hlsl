
float4 style(float2 uv, float g)
{
    STYLE_BRANCH if (far_away(uv, g))
        return float4(0, 0, 0, 0);
    float inside = body_at(uv);
    float ink = saturate(grown(uv, 3.0 * SIZE) - inside);
    float inner = inside * saturate(openness(uv, 2.0 * SIZE) * 3.0);
    float2 cell = frac(uv / TEXEL / (6.0 * SIZE)) - 0.5;
    float dots = inside * (1.0 - smoothstep(0.2, 0.28, length(cell))) * 0.18;
    return float4(c0.rgb * (inner * 1.2 + dots), ink);
}