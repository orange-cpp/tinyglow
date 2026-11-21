
float4 style(float2 uv, float g)
{
    return float4(c0.rgb * smoothstep(0.1, 0.16, g), 0);
}