
float4 style(float2 uv, float g)
{
    float col = STYLE_GRID_CELL(uv.x, TEXEL.x, 10.0 * SIZE);
    float speed = 0.3 + hash_fixed_row(col, 1.0) * 0.7;
    float d = frac(TIME * speed * 0.5 + hash_fixed_row(col, 2.0) - uv.y * 1.2);
    float trail = pow(1.0 - d, 8.0);
    float row = STYLE_GRID_CELL(uv.y, TEXEL.y, 12.0 * SIZE);
    float cell = step(0.45, hash(float2(col, row) + floor(TIME * 8.0)));
    return float4(c0.rgb * (trail * (0.4 + 0.6 * cell) * saturate(g * 2.5) * 1.6 + g * 0.15)
                  + tint(0.7) * pow(trail, 6.0) * cell * saturate(g * 2.5) * 0.6, 0);
}
