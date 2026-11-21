
float4 style(float2 uv, float g)
{
    STYLE_BRANCH if (g < 0.002)
        return float4(0, 0, 0, 0);
    float n = noise(uv * float2(60, 34) / SIZE + float2(TIME * 4.0, -TIME * 3.0));
    float arc = 1.0 - saturate(abs(n - 0.5) * 12.0);
    return float4((c0.rgb * 0.6 + 0.4) * arc * saturate(g * 2.5) * 2.0 + c0.rgb * g * 0.4, 0);
}