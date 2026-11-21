
float4 style(float2 uv, float g)
{
    STYLE_BRANCH if (far_away(uv, g))
        return float4(0, 0, 0, 0);
    float l = abs(frac(g * 8.0 / SIZE - TIME * 0.5) - 0.5);
    float lines = (1.0 - smoothstep(0.0, 0.12, l)) * smoothstep(0.0, 0.03, g);
    return float4(c0.rgb * (lines * 1.2 + g * 0.25), 0);
}
