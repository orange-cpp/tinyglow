
float4 composite_style(float2 uv)
{
    float4 o = style(uv, glow_at(uv));
    return float4(o.rgb * c2.x, saturate(o.a * min(c2.x, 1.0)));
}
