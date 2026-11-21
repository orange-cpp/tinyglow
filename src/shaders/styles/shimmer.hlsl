
float4 style(float2 uv, float g)
{
    float d = abs(frac((uv.x * 0.7 + uv.y * 0.4) / SIZE - TIME * 0.35) - 0.5);
    float sweep = 1.0 - smoothstep(0.0, 0.08, d);
    return float4(c0.rgb * g + sweep * g * 1.5, 0);
}