
float4 style(float2 uv, float g)
{
    float s = 0.5 + 0.5 * sin(uv.y * 900.0 / SIZE - TIME * 12.0);
    return float4(c0.rgb * g * (0.35 + 0.9 * s), 0);
}