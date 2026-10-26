// What the ASCII programs share. Their image starts with a strip of glyphs in the app's mono
// font drawn by scripts/generate-ascii-atlas.swift: twelve levels of light with two glyphs
// each, then the two falling specks use. A cell's light picks its level; every lit cell is a
// tile in its hue with the glyph over it, leaning to the ink as it brightens, and the
// brightest cells with a white core turn into a tile of ink with the glyph in the ground's
// colour. The build puts this after dither.wgsl in front of every program named `ascii_*`.

@group(2) @binding(0) var image: texture_2d<f32>;
@group(2) @binding(1) var image_sampler: sampler;

const CELL: vec2<f32> = vec2<f32>(9.0, 14.0);
const GLYPH_TEXELS: vec2<f32> = vec2<f32>(18.0, 28.0);
const LEVELS: f32 = 12.0;
const DASH: f32 = 24.0;
const MIDDLE_DOT: f32 = 25.0;
const GRID_ALPHA: f32 = 0.03;

struct Inks {
    ink: vec3<f32>,
    ground: vec3<f32>,
    // Colours a few cells flash in; none when `sparkles` is off.
    accents: array<vec3<f32>, 4>,
    sparkles: bool,
}

// A colour sent as one number, 0xRRGGBB, which a float holds exactly.
fn unpack(packed: f32) -> vec3<f32> {
    let v = u32(packed);
    return vec3<f32>(f32((v >> 16u) & 255u), f32((v >> 8u) & 255u), f32(v & 255u)) / 255.0;
}

fn hash3(x: u32, y: u32, z: u32) -> f32 {
    var h = (x * 374761393u) ^ (y * 668265263u) ^ (z * 1103515245u);
    h = (h ^ (h >> 13u)) * 1274126177u;
    h = h ^ (h >> 16u);
    return f32(h) / 4294967296.0;
}

fn image_texel(at: vec2<f32>, image_rect: vec4<f32>) -> vec4<f32> {
    let atlas = vec2<f32>(textureDimensions(image));
    return textureSampleLevel(image, image_sampler, (image_rect.xy + at) / atlas, 0.0);
}

fn glyph_cover(index: f32, inside: vec2<f32>, image_rect: vec4<f32>) -> f32 {
    let at = vec2<f32>(index * GLYPH_TEXELS.x, 0.0) + inside / CELL * GLYPH_TEXELS;
    return image_texel(at, image_rect).r;
}

fn over(top: vec4<f32>, under: vec4<f32>) -> vec4<f32> {
    return top + under * (1.0 - top.a);
}

// The faint line along each cell's top and left edges.
fn grid(inside: vec2<f32>, ink: vec3<f32>, scale: f32) -> vec4<f32> {
    let line = 1.0 / scale;
    if (inside.x < line || inside.y < line) {
        return vec4<f32>(ink * GRID_ALPHA, GRID_ALPHA);
    }
    return vec4<f32>(0.0);
}

// A lit cell's colour at `inside`, premultiplied: `light` from 0 to 1, `core` how much of
// it is white, `hue` its colour. Glyphs swap with their twin a little over once a second.
fn ascii_cell(
    inside: vec2<f32>,
    cell: vec2<f32>,
    light: f32,
    core: f32,
    hue: vec3<f32>,
    inks: Inks,
    t: f32,
    scale: f32,
    image_rect: vec4<f32>,
) -> vec4<f32> {
    var colour = grid(inside, inks.ink, scale);
    let level = min(LEVELS - 1.0, floor(light * LEVELS));
    let shade = (level + 0.5) / LEVELS;
    var white = 0.0;
    if (core >= 0.32) {
        white = 0.62;
    } else if (core >= 0.12) {
        white = 0.3;
    }
    let hot = level >= 10.0 && core >= 0.12;
    if (level >= 1.0 && all(inside >= vec2<f32>(1.0)) && all(inside < CELL - 1.0)) {
        var tile = hue * 0.58;
        var tile_alpha = 0.1 + shade * 0.5;
        if (hot) {
            tile = mix(tile, inks.ink, 0.5 + white * 0.3);
            tile_alpha = 0.9;
        }
        colour = over(vec4<f32>(tile * tile_alpha, tile_alpha), colour);
    }
    let x = u32(cell.x);
    let y = u32(cell.y);
    let tick = u32(floor(t * 1.4 + f32((x * 7u + y * 13u) & 15u) / 16.0));
    let h = hash3(x, y, tick);
    let cover = glyph_cover(level * 2.0 + select(1.0, 0.0, h < 0.55), inside, image_rect);
    var glyph = mix(hue, inks.ink, clamp(0.12 + shade * 0.5 + white * 0.34, 0.0, 0.94));
    var glyph_alpha = 0.4 + shade * 0.6;
    if (hot) {
        glyph = inks.ground;
        glyph_alpha = 1.0;
    } else if (inks.sparkles && level >= 4.0 && level < 10.0 && h > 0.986) {
        var accents = inks.accents;
        glyph = accents[u32(h * 10000.0) & 3u];
        glyph_alpha = 1.0;
    }
    let alpha = glyph_alpha * cover;
    return over(vec4<f32>(glyph * alpha, alpha), colour);
}

// The premultiplied colour, faded by `opacity`, as the renderer wants it.
fn ascii_out(colour: vec4<f32>, opacity: f32) -> vec4<f32> {
    let faded = colour * opacity;
    if (faded.a <= 0.002) {
        discard;
    }
    if (globals.premultiplied_alpha != 0u) {
        return faded;
    }
    return vec4<f32>(faded.rgb / faded.a, faded.a);
}
