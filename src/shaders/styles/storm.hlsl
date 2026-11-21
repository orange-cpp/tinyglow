
float4 style(float2 uv, float g)
{
    STYLE_BRANCH if (g < 0.002)
        return float4(0, 0, 0, 0);
    float n = fbm(square(uv) * 14.0 / SIZE + float2(floor(TIME * 12.0) * 7.13, 0.0));
    float bolt = 1.0 - saturate(abs(n - 0.5) * 28.0);
    float ring = saturate(g * 3.0) * (1.0 - saturate(g * 1.6 - 0.4));
    float flash = step(0.88, hash(float2(floor(TIME * 6.0), 1.7)));
    return float4(lerp(c0.rgb, float3(1, 1, 1), 0.6) * (bolt * ring * 2.5 + g * (0.25 + flash * 0.9)), 0);
}