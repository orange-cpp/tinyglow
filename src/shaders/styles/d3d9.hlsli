sampler2D s0 : register(s0);
sampler2D s1 : register(s1);
float4 c0 : register(c0);
float4 c1 : register(c1);
float4 c2 : register(c2);
float4 c3 : register(c3);

#define SAMPLE_GLOW(p) tex2Dlod(s0, float4(p, 0, 0)).r
#define SAMPLE_BODY(p) tex2Dlod(s1, float4(p, 0, 0)).r
#define STYLE_BRANCH [branch]
#define STYLE_UNROLL [unroll]
