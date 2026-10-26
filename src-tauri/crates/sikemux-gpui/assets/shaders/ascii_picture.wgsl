// The picture the person chose for their panes drawn in characters: each cell takes the
// picture's light under it, as an average over the cell, and its colour as the hue, covering
// the band at the top of a pane and fading out down it. It holds still. The picture sits in
// the image below the glyph strip.
//
// params: device pixels per point, the pane's corner radius in points and how strongly the
// picture shows; the ink and the ground as 0xRRGGBB, then the picture's width and height in
// texels.

fn fade_down(y: f32) -> f32 {
    if (y < 0.4) {
        return mix(1.0, 0.45, y / 0.4);
    }
    if (y < 0.76) {
        return mix(0.45, 0.08, (y - 0.4) / 0.36);
    }
    return mix(0.08, 0.0, min((y - 0.76) / 0.24, 1.0));
}

fn picture_at(point: vec2<f32>, size: vec2<f32>, texels: vec2<f32>, image_rect: vec4<f32>) -> vec4<f32> {
    let aspect = texels.x / texels.y;
    let covered = max(size.x / aspect, size.y) * vec2<f32>(aspect, 1.0);
    let uv = clamp((point - 0.5 * size) / covered + 0.5, vec2<f32>(0.0), vec2<f32>(1.0));
    return image_texel(vec2<f32>(0.0, GLYPH_TEXELS.y) + uv * (texels - 1.0) + 0.5, image_rect);
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

    let colours = shader.params[1];
    var inks: Inks;
    inks.ink = unpack(colours.x);
    inks.ground = unpack(colours.y);
    inks.sparkles = false;
    let texels = colours.zw;
    let cell = floor(local / CELL);
    let inside = local - cell * CELL;
    var texel = vec4<f32>(0.0);
    for (var tap = 0u; tap < 4u; tap = tap + 1u) {
        let offset = vec2<f32>(f32(tap & 1u), f32(tap >> 1u)) * 0.5 + 0.25;
        texel = texel + 0.25 * picture_at((cell + offset) * CELL, size, texels, shader.image);
    }
    let light = clamp(dot(vec3<f32>(0.2126, 0.7152, 0.0722), texel.rgb) * texel.a, 0.0, 1.0);
    let opacity = fade_down(local.y / size.y) * setup.z;
    if (light < 0.055) {
        return ascii_out(grid(inside, inks.ink, scale), opacity);
    }
    let hue = texel.rgb / max(max(texel.r, texel.g), max(texel.b, 0.05));
    let core = smoothstep(0.75, 1.0, light) * 0.4;
    return ascii_out(ascii_cell(inside, cell, light, core, hue, inks, 0.0, scale, shader.image), opacity);
}
