// What the ASCII programs share: a grid of character cells in points, twelve 5x7 glyphs
// from faint to dense, each packed as bits so no font is read, and how one cell is
// coloured: a dim tile in its own colour with the glyph over it in ink, and at the top of
// the ramp a tile of ink with the glyph cut out of it. The build puts it after dither.wgsl
// in front of every program whose name starts with `ascii_`.

const GLYPH_DOT: f32 = 1.2;
const CELL: vec2<f32> = vec2<f32>(6.0 * GLYPH_DOT, 9.0 * GLYPH_DOT);
const GLYPHS: u32 = 12u;
const SOLID: f32 = 0.92;
const GRID_ALPHA: f32 = 0.05;

// The glyph's rows top to bottom, five bits each from the left: rows 0 to 3, then 4 to 6.
fn glyph_bits(index: u32) -> vec2<u32> {
    var ramp = array<vec2<u32>, 12>(
        vec2<u32>(0u, 128u),
        vec2<u32>(132u, 0u),
        vec2<u32>(128u, 128u),
        vec2<u32>(128u, 2180u),
        vec2<u32>(571392u, 14897u),
        vec2<u32>(162948u, 24708u),
        vec2<u32>(139891u, 26402u),
        vec2<u32>(459838u, 15888u),
        vec2<u32>(141873u, 17962u),
        vec2<u32>(476718u, 14897u),
        vec2<u32>(359754u, 10591u),
        vec2<u32>(718382u, 30781u),
    );
    return ramp[min(index, GLYPHS - 1u)];
}

// Whether the glyph for `light` has a stroke at `inside`, in points from its cell's top
// left corner. The glyph sits one dot in from the cell's top and leaves its right column.
fn glyph_on(inside: vec2<f32>, light: f32) -> bool {
    let dot_at = vec2<i32>(floor(inside / GLYPH_DOT)) - vec2<i32>(0, 1);
    if (dot_at.x < 0 || dot_at.x >= 5 || dot_at.y < 0 || dot_at.y >= 7) {
        return false;
    }
    let bits = glyph_bits(u32(clamp(light, 0.0, 0.999) * f32(GLYPHS)));
    let bit = u32(dot_at.y * 5 + dot_at.x);
    let word = select(bits.x >> bit, bits.y >> (bit - 20u), bit >= 20u);
    return (word & 1u) == 1u;
}

// A cell's colour at `inside`, premultiplied: the grid line along its top and left edges,
// then the tile, then the glyph. `light` from 0 to 1 picks the glyph and how bright both are.
fn ascii_cell(inside: vec2<f32>, light: f32, tile: vec3<f32>, ink: vec3<f32>, scale: f32) -> vec4<f32> {
    let line = 1.0 / scale;
    let on_grid = inside.x < line || inside.y < line;
    let grid = vec4<f32>(ink * GRID_ALPHA, GRID_ALPHA);
    if (light <= 0.04) {
        return select(vec4<f32>(0.0), grid, on_grid);
    }
    if (on_grid) {
        return grid;
    }
    let stroke = glyph_on(inside, light);
    if (light > SOLID) {
        let colour = select(ink, tile * 0.35, stroke);
        return vec4<f32>(colour * 0.92, 0.92);
    }
    let tile_alpha = 0.16 + 0.42 * light;
    if (!stroke) {
        return vec4<f32>(tile * tile_alpha, tile_alpha);
    }
    let glyph = mix(tile, ink, 0.55 + 0.4 * light);
    let glyph_alpha = 0.35 + 0.6 * light;
    let alpha = glyph_alpha + tile_alpha * (1.0 - glyph_alpha);
    return vec4<f32>(glyph * glyph_alpha + tile * tile_alpha * (1.0 - glyph_alpha), alpha);
}

fn cell_hash(cell: vec2<f32>) -> f32 {
    return fract(sin(dot(cell, vec2<f32>(127.1, 311.7))) * 43758.5453);
}

// The premultiplied colour, faded by `opacity`, as the renderer wants it.
fn ascii_out(colour: vec4<f32>, opacity: f32) -> vec4<f32> {
    let faded = colour * opacity;
    if (faded.a <= 0.0) {
        discard;
    }
    if (globals.premultiplied_alpha != 0u) {
        return faded;
    }
    return vec4<f32>(faded.rgb / faded.a, faded.a);
}
