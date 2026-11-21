
float4 style(float2 uv, float g)
{
    float on = step(0.5, frac(TIME * 4.0));
    return float4(lerp(c0.rgb, hue_turn(0.5), on) * g * 1.2, 0);
}