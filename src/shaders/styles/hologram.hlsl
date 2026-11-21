
float4 style(float2 uv, float g)
{
    float tear = (hash(float2(floor(TIME * 20.0), floor(uv.y * 60.0))) - 0.5) * 0.006
               * step(0.9, hash(float2(floor(TIME * 8.0), 3.0)));
    float m = glow_at(uv + float2(tear, 0.0));
    float lines = 0.6 + 0.4 * sin(uv.y / TEXEL.y * 1.6 / SIZE);
    float band = 1.0 - smoothstep(0.0, 0.05, abs(frac(uv.y * 0.6 - TIME * 0.25) - 0.5));
    float hum = 0.88 + 0.12 * sin(TIME * 50.0);
    return float4(tint(0.25) * m * (lines + band * 1.3) * hum, 0);
}