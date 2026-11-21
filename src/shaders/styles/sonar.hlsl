
float4 style(float2 uv, float g)
{
    float ping = frac(TIME * 0.6);
    float ring = (1.0 - smoothstep(0.0, 0.06 * SIZE, abs(g - (1.0 - ping)))) * (1.0 - ping) * step(0.02, g);
    return float4(c0.rgb * (ring * 1.8 + g * 0.3), 0);
}