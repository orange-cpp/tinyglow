// Shared by all Direct3D profiles. Tinyglow uses a single red coverage channel.
static const float offsets[6] = {1.3366, 3.4295, 5.4264, 7.4359, 9.4436, 11.4401};
static const float weights[6] = {0.2185, 0.0821, 0.0461, 0.0262, 0.0162, 0.0102};

#ifdef SHADER_MODEL_LEGACY

sampler2D input_texture : register(s0);
float4 parameters : register(c0);
#define SAMPLE_BLUR(uv) tex2D(input_texture, uv).r
#define SAMPLE_COMPOSITE(uv) tex2Dlod(input_texture, float4(uv, 0, 0)).r
#define PIXEL_OUTPUT COLOR0
#define PIXEL_INPUT float2 uv : TEXCOORD0

struct Vertex {
    float4 position : POSITION;
    float2 uv : TEXCOORD0;
};

Vertex legacy_vertex(Vertex vertex)
{
    return vertex;
}

#define BLUR_ENTRY legacy_blur
#define COMPOSITE_ENTRY legacy_composite

#else

Texture2D input_texture : register(t0);
SamplerState linear_clamp : register(s0);
cbuffer ClassicConstants : register(b0) { float4 parameters; }
#define SAMPLE_BLUR(uv) input_texture.Sample(linear_clamp, uv).r
#define SAMPLE_COMPOSITE(uv) input_texture.SampleLevel(linear_clamp, uv, 0).r
#define PIXEL_OUTPUT SV_Target0
#define PIXEL_INPUT float4 position : SV_Position, float2 uv : TEXCOORD0

struct Vertex {
    float4 position : SV_Position;
    float2 uv : TEXCOORD0;
};

Vertex fullscreen_vertex(uint vertex : SV_VertexID)
{
    Vertex result;
    result.uv = float2((vertex << 1) & 2, vertex & 2);
    result.position = float4(result.uv * float2(2, -2) + float2(-1, 1), 0, 1);
    return result;
}

Vertex fullscreen_quad(uint vertex : SV_VertexID)
{
    Vertex result;
    result.uv = float2(vertex & 1, (vertex >> 1) & 1);
    result.position = float4(result.uv * float2(2, -2) + float2(-1, 1), 0, 1);
    return result;
}

#define BLUR_ENTRY blur_fragment
#define COMPOSITE_ENTRY composite_fragment

#endif

float4 BLUR_ENTRY(PIXEL_INPUT) : PIXEL_OUTPUT
{
    float coverage = SAMPLE_BLUR(uv) * 0.2013;
    [unroll] for (int i = 0; i < 6; ++i) {
        float2 delta = parameters.xy * offsets[i];
        coverage += (SAMPLE_BLUR(uv + delta) + SAMPLE_BLUR(uv - delta)) * weights[i];
    }
    return float4((coverage * parameters.z).xxx, 1);
}

float4 COMPOSITE_ENTRY(PIXEL_INPUT) : PIXEL_OUTPUT
{
    float coverage = SAMPLE_COMPOSITE(uv);
    return float4((parameters.rgb * coverage) * parameters.a, 0);
}
