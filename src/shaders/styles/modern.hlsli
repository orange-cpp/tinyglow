Texture2D<float4> blurred : register(t0);
Texture2D<float4> silhouette : register(t1);
SamplerState linear_clamp : register(s0);
cbuffer StyleConstants : register(b0) {
    float4 c0;
    float4 c1;
    float4 c2;
    float4 c3;
};

#define SAMPLE_GLOW(p) blurred.SampleLevel(linear_clamp, p, 0).r
#define SAMPLE_BODY(p) silhouette.SampleLevel(linear_clamp, p, 0).r
#define STYLE_BRANCH [branch]
#define STYLE_UNROLL [unroll]
#define STYLE_RANGE_REDUCE
#define STYLE_MAD mad
