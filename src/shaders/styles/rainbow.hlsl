
float4 style(float2 uv, float g)
{
    float h = frac((uv.x * 1.5 + uv.y * 0.5) / SIZE - TIME * 0.3);
    return float4(hue_turn(h) * saturate(g * 1.5), 0);
}