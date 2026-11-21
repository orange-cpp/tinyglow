
float4 style(float2 uv, float g)
{
    float n = hash(floor(uv * float2(160, 90) / SIZE) + floor(TIME * 14.0));
    return float4(c0.rgb * g * (0.55 + 0.7 * n), 0);
}