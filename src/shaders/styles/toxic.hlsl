
float4 style(float2 uv, float g)
{
    STYLE_BRANCH if (g < 0.002)
        return float4(0, 0, 0, 0);
    float2 p = square(uv) * 18.0 / SIZE + float2(0.0, TIME * 0.8);
    float sludge = noise(p);
    float bubbles = smoothstep(0.62, 0.68, noise(p * 1.8 + TIME * 0.5));
    return float4(c0.rgb * g * (0.5 + 0.7 * sludge) + tint(0.5) * bubbles * saturate(g * 3.0) * 0.8, 0);
}