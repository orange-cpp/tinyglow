
float4 style(float2 uv, float g)
{
    float2 p = uv / SIZE;
    float h = (frac(p.y * 0.8 + sin(p.x * 6.0 + TIME) * 0.1 + TIME * 0.05) - 0.5) * 0.3;
    float ripple = 0.7 + 0.3 * sin(p.x * 30.0 + TIME * 3.0);
    return float4(hsv(c3.x + h, c3.y * 0.85, c3.z) * saturate(g * 1.5) * ripple, 0);
}