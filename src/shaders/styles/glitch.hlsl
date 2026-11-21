
float4 style(float2 uv, float g)
{
    float row = floor(uv.y * 40.0 / SIZE);
    float t = floor(TIME * 10.0);
    float shift = (hash(float2(row, t)) - 0.5) * step(0.8, hash(float2(t, row * 0.37))) * 0.03 * SIZE;
    float s = glow_at(uv + float2(shift, 0));
    return float4(c0.rgb * s * 1.1 + hue_turn(0.5) * 0.4 * abs(shift / SIZE) * 30.0 * s, 0);
}