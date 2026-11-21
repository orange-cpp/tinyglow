
float4 style(float2 uv, float g)
{
    STYLE_BRANCH if (g < 0.002)
        return float4(0, 0, 0, 0);
    float2 p = square(uv) * 26.0 / SIZE;
    float crystal = noise(p + hash(floor(p)) * 4.0);
    float glint = step(0.985, hash(floor(uv / TEXEL * 0.5 / SIZE) + floor(TIME * 3.0)));
    float3 ice = lerp(tint(0.3), float3(1, 1, 1), crystal);
    float edge = smoothstep(0.05, 0.25, g);
    return float4(ice * (g * 0.7 + edge * crystal * 0.6) + glint * saturate(g * 3.0) * 2.0, 0);
}