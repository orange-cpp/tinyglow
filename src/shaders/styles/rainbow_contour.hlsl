
float4 style(float2 uv, float g)
{
    float h = frac(g * 1.2 / SIZE - TIME * 0.4);
    return float4(hue_turn(h) * saturate(g * 1.4), 0);
}