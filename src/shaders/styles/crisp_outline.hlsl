
float4 style(float2 uv, float g)
{
    STYLE_BRANCH if (far_away(uv, g))
        return float4(0, 0, 0, 0);
    float ring = saturate(grown(uv, 2.0 * SIZE) - body_at(uv));
    return float4(c0.rgb * ring, ring);
}