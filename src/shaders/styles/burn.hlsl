
float4 style(float2 uv, float g)
{
    STYLE_BRANCH if (far_away(uv, g))
        return float4(0, 0, 0, 0);
    float inside = body_at(uv);
    float progress = frac(TIME * 0.09) * 1.25;
    float n = fbm(square(uv) * 9.0 / SIZE);
    float edge = 1.0 - smoothstep(0.0, 0.05, abs(n - progress));
    float burnt = step(n, progress);
    float3 ember = heat(0.9) * edge * 2.0;
    return float4(ember * inside + c0.rgb * g * 0.2 * (1.0 - inside), burnt * inside * 0.85);
}