// 灵栖渲染内核的着色器。
//
// 改写自 egui-wgpu 的 egui.wgsl（MIT / Apache-2.0，见 THIRD_PARTY_NOTICES）：
// 顶点位置从逻辑点换算到裁剪空间，颜色按 sRGB 通道解包，片元里把纹理取样与
// 顶点颜色相乘，并按需要做伽马/线性空间的往返转换。
//
// 顶点输入：0 位置、1 uv、2 颜色（u32 打包的 sRGBA）。

struct VertexOutput {
    @location(0) tex_coord: vec2<f32>,
    @location(1) color: vec4<f32>, // 伽马空间 0-1
    @builtin(position) position: vec4<f32>,
};

struct Locals {
    screen_size: vec2<f32>,
    dithering: u32, // 1 = 开抖动
    _padding: u32,
};

@group(0) @binding(0) var<uniform> r_locals: Locals;

@group(1) @binding(0) var r_tex_color: texture_2d<f32>;
@group(1) @binding(1) var r_tex_sampler: sampler;

// 交错梯度噪声：把 8 位色阶的条带打散成细微噪点。
fn interleaved_gradient_noise(n: vec2<f32>) -> f32 {
    let f = 0.06711056 * n.x + 0.00583715 * n.y;
    return fract(52.9829189 * fract(f));
}

fn dither_interleaved(rgb: vec3<f32>, levels: f32, frag_coord: vec4<f32>) -> vec3<f32> {
    var noise = interleaved_gradient_noise(frag_coord.xy);
    // 幅度压到 0.95，避免纯色被抖出可见的颗粒。
    noise = (noise - 0.5) * 0.95;
    return rgb + noise / (levels - 1.0);
}

// sRGB 伽马 → 线性
fn linear_from_gamma_rgb(srgb: vec3<f32>) -> vec3<f32> {
    let cutoff = srgb < vec3<f32>(0.04045);
    let lower = srgb / vec3<f32>(12.92);
    let higher = pow((srgb + vec3<f32>(0.055)) / vec3<f32>(1.055), vec3<f32>(2.4));
    return select(higher, lower, cutoff);
}

// 线性 → sRGB 伽马
fn gamma_from_linear_rgb(rgb: vec3<f32>) -> vec3<f32> {
    let cutoff = rgb < vec3<f32>(0.0031308);
    let lower = rgb * vec3<f32>(12.92);
    let higher = vec3<f32>(1.055) * pow(rgb, vec3<f32>(1.0 / 2.4)) - vec3<f32>(0.055);
    return select(higher, lower, cutoff);
}

fn gamma_from_linear_rgba(linear_rgba: vec4<f32>) -> vec4<f32> {
    return vec4<f32>(gamma_from_linear_rgb(linear_rgba.rgb), linear_rgba.a);
}

// [u8; 4] sRGB 打包成 u32 → [r, g, b, a]（0-1）
fn unpack_color(color: u32) -> vec4<f32> {
    return vec4<f32>(
        f32(color & 255u),
        f32((color >> 8u) & 255u),
        f32((color >> 16u) & 255u),
        f32((color >> 24u) & 255u),
    ) / 255.0;
}

fn position_from_screen(screen_pos: vec2<f32>) -> vec4<f32> {
    return vec4<f32>(
        2.0 * screen_pos.x / r_locals.screen_size.x - 1.0,
        1.0 - 2.0 * screen_pos.y / r_locals.screen_size.y,
        0.0,
        1.0,
    );
}

@vertex
fn vs_main(
    @location(0) a_pos: vec2<f32>,
    @location(1) a_tex_coord: vec2<f32>,
    @location(2) a_color: u32,
) -> VertexOutput {
    var out: VertexOutput;
    out.tex_coord = a_tex_coord;
    out.color = unpack_color(a_color);
    out.position = position_from_screen(a_pos);
    return out;
}

// 画到伽马空间的目标（推荐）：颜色原样写出。
@fragment
fn fs_main_gamma_framebuffer(in: VertexOutput) -> @location(0) vec4<f32> {
    let tex_linear = textureSample(r_tex_color, r_tex_sampler, in.tex_coord);
    let tex_gamma = gamma_from_linear_rgba(tex_linear);
    var out_color_gamma = in.color * tex_gamma;
    // 抖动在伽马空间做，与 8 位目标配合更好。
    if r_locals.dithering == 1 {
        let out_color_gamma_rgb = dither_interleaved(out_color_gamma.rgb, 256.0, in.position);
        out_color_gamma = vec4<f32>(out_color_gamma_rgb, out_color_gamma.a);
    }
    return out_color_gamma;
}

// 画到线性空间的目标：写出前转成线性。
@fragment
fn fs_main_linear_framebuffer(in: VertexOutput) -> @location(0) vec4<f32> {
    let tex_linear = textureSample(r_tex_color, r_tex_sampler, in.tex_coord);
    let tex_gamma = gamma_from_linear_rgba(tex_linear);
    var out_color_gamma = in.color * tex_gamma;
    if r_locals.dithering == 1 {
        let out_color_gamma_rgb = dither_interleaved(out_color_gamma.rgb, 256.0, in.position);
        out_color_gamma = vec4<f32>(out_color_gamma_rgb, out_color_gamma.a);
    }
    let out_color_linear = linear_from_gamma_rgb(out_color_gamma.rgb);
    return vec4<f32>(out_color_linear, out_color_gamma.a);
}
