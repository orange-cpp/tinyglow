
float4 style(float2 uv, float g)
{
    float2 p = uv / SIZE;
    float v = sin(p.x * 20.0 + TIME * 2.0) + sin(p.y * 16.0 - TIME * 1.7) + sin((p.x + p.y) * 14.0 + TIME * 1.3);
    return float4(hsv(c3.x + frac(v * 0.15 + TIME * 0.1), c3.y * 0.8, c3.z) * saturate(g * 1.4), 0);
}