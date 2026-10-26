// The picture the person chose for their panes drawn in characters: each cell takes the
// picture's light under it as a glyph, on a tile in the picture's own colour with the glyph
// leaning to the theme's ink, covering the band at the top of a pane and fading out down
// it. It holds still.
//
// params: device pixels per point, the pane's corner radius in points and how strongly
// the picture shows; the ink.

@group(2) @binding(0) var image: texture_2d<f32>;
@group(2) @binding(1) var image_sampler: sampler;

fn fade_down(y: f32) -> f32 {
    if (y < 0.4) {
        return mix(1.0, 0.45, y / 0.4);
    }
    if (y < 0.76) {
        return mix(0.45, 0.08, (y - 0.4) / 0.36);
    }
    return mix(0.08, 0.0, min((y - 0.76) / 0.24, 1.0));
}

@fragment
fn fs_main(input: Varying) -> @location(0) vec4<f32> {
    let shader = instances[input.instance];
    let device = input.position.xy;
    if (any(device < shader.clip.xy) || any(device >= shader.clip.xy + shader.clip.zw)) {
        discard;
    }
    let setup = shader.params[0];
    let scale = setup.x;
    let size = shader.bounds.zw / scale;
    let local = (device - shader.bounds.xy) / scale;
    // Only the band's top corners are the pane's; its bottom edge runs across the pane.
    let rounded = vec2<f32>(size.x, size.y + 2.0 * setup.y);
    if (outside_corners(local, rounded, setup.y)) {
        discard;
    }

    let cell = floor(local / CELL);
    let inside = local - cell * CELL;
    let texels = shader.image.zw;
    let aspect = texels.x / texels.y;
    let covered = max(size.x / aspect, size.y) * vec2<f32>(aspect, 1.0);
    let atlas = vec2<f32>(textureDimensions(image));
    var texel = vec4<f32>(0.0);
    for (var tap = 0u; tap < 4u; tap = tap + 1u) {
        let offset = vec2<f32>(f32(tap & 1u), f32(tap >> 1u)) * 0.5 + 0.25;
        let at = ((cell + offset) * CELL - 0.5 * size) / covered + 0.5;
        let uv = clamp(at, vec2<f32>(0.0), vec2<f32>(1.0));
        texel = texel + 0.25 * textureSampleLevel(image, image_sampler, (shader.image.xy + uv * texels) / atlas, 0.0);
    }

    let lum = dot(vec3<f32>(0.2126, 0.7152, 0.0722), texel.rgb);
    let light = clamp(lum * texel.a, 0.0, 1.0);
    let brightest = max(max(texel.r, texel.g), max(texel.b, 0.05));
    let tile = clamp(texel.rgb / brightest, vec3<f32>(0.0), vec3<f32>(1.0));
    let ink = mix(tile, shader.params[1].rgb, 0.6);
    return ascii_out(ascii_cell(inside, light, tile, ink, scale), fade_down(local.y / size.y) * setup.z);
}
