
float4 style(float2 uv, float g)
{
    STYLE_BRANCH if (far_away(uv, g))
        return float4(0, 0, 0, 0);
    float inside = body_at(uv);
    float2 p = square(uv) * 45.0 / SIZE;
    float2 r = float2(1.0, 1.7320508);
    float2 a = fmod(p, r) - r * 0.5;
    float2 b = fmod(p - r * 0.5, r) - r * 0.5;
    float2 cell = dot(a, a) < dot(b, b) ? a : b;
    float hex = max(abs(cell.x), dot(abs(cell), float2(0.5, 0.8660254)));
    float lines = smoothstep(0.40, 0.48, hex);
    float scan = 1.0 - smoothstep(0.0, 0.06, abs(frac(uv.y * 0.8 - TIME * 0.35) - 0.5));
    float rim = saturate(openness(uv, 3.0 * SIZE) * 2.5);
    float shell = inside * (lines * (0.35 + scan * 0.9) + rim * 0.9 + 0.06) + (1.0 - inside) * g * 0.5;
    return float4(tint(0.3) * shell, 0);
}