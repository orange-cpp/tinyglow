#define TIME c1.x
#define TEXEL c1.yz
#define SIZE c2.y
#ifndef STYLE_GRID_CELL
#define STYLE_GRID_CELL(p, texel, spacing) floor((p) / (texel) / (spacing))
#endif

float glow_at(float2 p)
{
    return SAMPLE_GLOW(p);
}

float body_at(float2 p)
{
    return SAMPLE_BODY(p);
}

float3 hsv(float h, float s, float v)
{
    float3 k = float3(1.0, 2.0 / 3.0, 1.0 / 3.0);
    float3 p = abs(frac(h + k) * 6.0 - 3.0);
    return v * lerp(float3(1, 1, 1), saturate(p - 1.0), s);
}

// Every style's palette is built on the group's colour (c0 / c3):
// the group's colour with its hue turned by d (1 = a full turn), its saturation and value kept
float3 hue_turn(float d)
{
    return hsv(c3.x + d, c3.y, c3.z);
}

// the group's colour paled towards white by k
float3 tint(float k)
{
    return lerp(c0.rgb, float3(1, 1, 1), k);
}

// a fire's colours over the group's colour: its deep shade (the hue turned back a little), the colour, a pale tint of
// it, white-hot.  h = the heat, 0 .. ~1.5
float3 heat(float h)
{
    float k = h * 2.5;
    return hue_turn(-0.04) * 0.6 * saturate(k) + c0.rgb * 0.35 * saturate(k - 0.6) + tint(0.6) * 0.3 * saturate(k - 1.3)
         + 0.3 * saturate(k - 2.0);
}

#ifdef STYLE_RANGE_REDUCE
float hash_phase(float phase)
{
    // Shader Model 3's compiler emits this reduction before sincos. Tiny
    // phase differences are amplified by the hash multiplier.
    precise float reduced = STYLE_MAD(frac(STYLE_MAD(phase, 0.159154937, 0.5)), 6.28318548, -3.14159274);
    precise float value = sin(reduced) * 43758.5453;
    return frac(value);
}

float hash(float2 p)
{
    precise float phase = STYLE_MAD(p.y, 78.233, p.x * 12.9898);
    return hash_phase(phase);
}

// When one coordinate is constant, the legacy compiler folds its product
// into the addend before it evaluates the other coordinate.
float hash_fixed_row(float x, float y)
{
    precise float phase = STYLE_MAD(x, 12.9898, y * 78.233);
    return hash_phase(phase);
}
#else
float hash(float2 p)
{
    return frac(sin(dot(p, float2(12.9898, 78.233))) * 43758.5453);
}
#define hash_fixed_row(x, y) hash(float2(x, y))
#endif

// value noise, smooth, 0 .. 1
float noise(float2 p)
{
    float2 i = floor(p);
    float2 f = frac(p);
    f = f * f * (3.0 - 2.0 * f);
    float n0 = lerp(hash(i), hash(i + float2(1, 0)), f.x);
    float n1 = lerp(hash(i + float2(0, 1)), hash(i + float2(1, 1)), f.x);
    return lerp(n0, n1, f.y);
}

// two octaves of noise: turbulence
float fbm(float2 p)
{
    return noise(p) * 0.65 + noise(p * 2.1 + 17.3) * 0.35;
}

// screen-proportional coordinates (x scaled by width / height), so noise cells are square
float2 square(float2 uv)
{
    return uv * float2(c1.z / c1.y, 1.0);
}

// nothing of a model within the glow's reach: the cheap styles stop here
bool far_away(float2 uv, float g)
{
    return g < 0.002 && body_at(uv) < 0.5;
}

// the sharp silhouette grown by r mask pixels (8 directions, at r and r / 2): 1 on the model and within r of it
float grown(float2 uv, float r)
{
    float m = body_at(uv);
    STYLE_UNROLL for (int i = 0; i < 8; i++)
    {
        float a = i * 0.7853982;
        float2 d = float2(cos(a), sin(a)) * TEXEL;
        m = max(m, max(body_at(uv + d * r), body_at(uv + d * r * 0.5)));
    }
    return m;
}

// how much of the ring of radius r (mask pixels) around the pixel is off the model: 0 deep inside, ~0.5 on the edge
float openness(float2 uv, float r)
{
    float open = 0.0;
    STYLE_UNROLL for (int i = 0; i < 8; i++)
    {
        float a = i * 0.7853982;
        open += 1.0 - body_at(uv + float2(cos(a), sin(a)) * TEXEL * r);
    }
    return open / 8.0;
}

// Flames rising off the silhouette.  The glow is sampled below the pixel at 6 heights, each bent sideways by a
// turbulence field that scrolls upwards, so the outline is dragged up into licking tongues; a second, faster noise cuts
// the result into separate flames.  0 .. ~1.5 (the heat).  `reach` is a fraction of the screen height (times Size).
float flames(float2 uv, float g, float speed, float reach)
{
    reach *= SIZE;
    float probe = max(max(glow_at(uv + float2(0.0, reach * 0.5)), glow_at(uv + float2(0.0, reach))),
                      max(glow_at(uv + float2(0.02, reach * 0.6)), glow_at(uv + float2(-0.02, reach * 0.6))));
    STYLE_BRANCH if (max(g, probe) < 0.003)
        return 0.0;
    float2 p = square(uv) * 7.0 / SIZE;
    float t = TIME * speed;
    float bend = fbm(p + float2(0.0, t * 1.6)) - 0.5;
    float lift = 0.0;
    STYLE_UNROLL for (int i = 1; i <= 6; i++)
    {
        float k = float(i) / 6.0;
        lift = max(lift, glow_at(uv + float2(bend * 0.035 * SIZE * k, reach * k)) * (1.0 - k * 0.8));
    }
    float body = max(g, lift);
    float tongues = fbm(p * float2(1.6, 0.9) + float2(0.0, t * 3.2));
    return saturate((body * 1.5 + tongues - 0.95) * 2.4) * (0.4 + body);
}

// Sparks flying up out of a fire: a grid scrolling upwards (with a sway), one hashed point per cell, one cell in six
// lit, twinkling.  `area` says where they may appear (0 .. 1).
float embers(float2 uv, float area)
{
    float2 grid = square(uv) * 55.0 / SIZE + float2(0.0, TIME * 7.0);
    grid.x += sin(grid.y * 0.3 + TIME * 1.3) * 0.7;
    float2 cell = floor(grid);
    float2 at = float2(hash(cell + 3.1), hash(cell + 7.7)) - 0.5;
    float spark = 1.0 - smoothstep(0.03, 0.13, length(frac(grid) - 0.5 - at * 0.6));
    float lit = step(0.83, hash(cell)) * (0.6 + 0.4 * sin(TIME * 20.0 + hash(cell + 1.9) * 50.0));
    return spark * lit * area;
}

// Smoke above a fire: a domain-warped noise that drifts up, fed by the glow below the pixel.  0 .. 1 (the density).
float smoke(float2 uv, float g, float reach)
{
    reach *= SIZE;
    float source = max(g, max(glow_at(uv + float2(0.0, reach)), glow_at(uv + float2(0.012, reach * 1.8))));
    STYLE_BRANCH if (source < 0.003)
        return 0.0;
    float2 p = square(uv) * 4.5 / SIZE + float2(0.0, TIME * 0.5);
    float n = fbm(p + fbm(p * 0.7 - TIME * 0.2) * 1.5);
    return saturate(source * 1.4) * saturate(n * 1.6 - 0.35);
}
