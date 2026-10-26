// A pane's surface drawn in characters: three jellyfish pulsing and swaying over sparse
// drifting noise, each cell's brightness picking its glyph. Tiles drift between the
// theme's two hues, a few cells flash in the second, and glyphs are in the theme's ink.
// Like the dithered grain it fades out by the middle of the pane.
//
// params: where the clock starts, how fast it runs, device pixels per point and the
// pane's corner radius in points; the first hue with how strongly it all shows; the
// second hue; the ink.

const TENTACLES: i32 = 7;

fn fade_down(y: f32) -> f32 {
    if (y < 0.25) {
        return mix(1.0, 0.5, y / 0.25);
    }
    if (y < 0.45) {
        return mix(0.5, 0.1, (y - 0.25) / 0.2);
    }
    return mix(0.1, 0.0, min((y - 0.45) / 0.15, 1.0));
}

// One jellyfish's light at `p`, in points with y down, its bell's top centred on `at`.
fn jelly(p: vec2<f32>, at: vec2<f32>, radius: f32, t: f32, seed: f32) -> f32 {
    let pulse = sin(t * 1.6 + seed * 6.0);
    let sway = vec2<f32>(sin(t * 0.35 + seed * 9.0) * radius * 0.35, cos(t * 0.27 + seed) * radius * 0.15);
    let centre = at + sway;
    let bell = (p - centre) / (radius * vec2<f32>(1.0 + 0.07 * pulse, 0.78 - 0.07 * pulse));
    let reach = length(bell);
    var light = 0.0;
    let frill = 0.22 + 0.1 * sin(bell.x * 11.0 + t * 2.0 + seed * 5.0);
    if (bell.y < frill && reach < 1.0) {
        let rim = smoothstep(0.55, 0.95, reach);
        let crown = clamp(-bell.y, 0.0, 1.0);
        let ripple = 0.5 + 0.5 * snoise(bell * 2.2 + vec2<f32>(seed * 13.0, t * 0.4));
        let hollow = smoothstep(-0.1, 0.3, bell.y) * (1.0 - rim);
        light = 0.2 + 0.25 * crown + 0.3 * rim + 0.4 * ripple * ripple - 0.25 * hollow;
    }
    let below = (p.y - centre.y) / radius - 0.25;
    if (below > 0.0 && below < 3.0) {
        for (var k = 0; k < TENTACLES; k = k + 1) {
            let strand_seed = fract(f32(k) * 0.618 + seed);
            let strand_length = 1.4 + 1.6 * strand_seed;
            if (below > strand_length) {
                continue;
            }
            let across = (f32(k) / f32(TENTACLES - 1) - 0.5) * 1.5;
            let wave = sin(below * 2.6 - t * 1.8 + strand_seed * 6.3) * 0.25 * below;
            let x = centre.x + radius * (across * (1.0 - 0.12 * below) + wave);
            let width = radius * 0.08;
            let strand = 1.0 - smoothstep(width * 0.4, width, abs(p.x - x));
            light = max(light, strand * (0.75 - 0.6 * below / strand_length));
        }
    }
    return clamp(light, 0.0, 1.0);
}

@fragment
fn fs_main(input: Varying) -> @location(0) vec4<f32> {
    let shader = instances[input.instance];
    let device = input.position.xy;
    if (any(device < shader.clip.xy) || any(device >= shader.clip.xy + shader.clip.zw)) {
        discard;
    }
    let setup = shader.params[0];
    let t = setup.x + globals.time * setup.y;
    let scale = setup.z;
    let size = shader.bounds.zw / scale;
    let local = (device - shader.bounds.xy) / scale;
    let fade = fade_down(local.y / size.y);
    if (fade <= 0.0) {
        discard;
    }
    if (outside_corners(local, size, setup.w)) {
        discard;
    }

    let cell = floor(local / CELL);
    let inside = local - cell * CELL;
    let p = (cell + 0.5) * CELL;
    let radius = clamp(min(size.x, size.y) * 0.15, 32.0, 110.0);
    var light = jelly(p, vec2<f32>(0.2 * size.x, radius + 0.06 * size.y), radius, t, 0.13);
    light = max(light, jelly(p, vec2<f32>(0.56 * size.x, radius * 1.15 + 0.02 * size.y), radius * 1.15, t, 0.61));
    light = max(light, jelly(p, vec2<f32>(0.85 * size.x, radius * 0.85 + 0.14 * size.y), radius * 0.85, t, 0.37));
    let drift = snoise(p * 0.004 + vec2<f32>(0.0, -0.06 * t)) + 0.6 * snoise(p * 0.011 + vec2<f32>(0.05 * t, 0.0));
    let specks = smoothstep(0.2, 1.1, drift) * 0.35;
    if (cell_hash(cell) < specks) {
        light = max(light, 0.08 + 0.2 * cell_hash(cell + 7.0));
    }

    let tiles = shader.params[1];
    let sparkle = shader.params[2];
    let ink = shader.params[3].rgb;
    let blend = smoothstep(-0.6, 0.6, snoise(p * 0.0025 + vec2<f32>(0.03 * t, 0.0)));
    var tile = mix(tiles.rgb, sparkle.rgb, blend * 0.55);
    var shade = ink;
    if (light > 0.3 && cell_hash(cell + floor(t * 0.5)) > 0.985) {
        tile = sparkle.rgb;
        shade = sparkle.rgb;
    }
    return ascii_out(ascii_cell(inside, light, tile, shade, scale), tiles.a * fade);
}
