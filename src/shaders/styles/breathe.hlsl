
float4 style(float2 uv, float g)
{
    float k = 0.15 + 0.85 * (0.5 + 0.5 * sin(TIME * 2.5));
    return float4(c0.rgb * g * k * 1.2, 0);
}