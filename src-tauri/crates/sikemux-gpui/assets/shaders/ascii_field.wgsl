// A pane's surface drawn in characters: moon jellyfish from the image's sprite swimming up
// through the pane, each pulse of the bell pushing it on, its tentacles swaying behind, with
// specks falling past. A jellyfish leans from the theme's first hue to its second, and when one
// leaves past the top it comes back from below somewhere else. Everything follows from the
// clock alone, so nothing is kept between frames. It fades out down the pane.
//
// params: where the clock starts, how fast it runs, device pixels per point and the pane's
// corner radius in points; the first hue, the second, the ink and the ground, each as
// 0xRRGGBB; four colours a few cells flash in; how strongly it all shows.

const SPRITE_TOP: f32 = 28.0;
const SPRITE: vec2<f32> = vec2<f32>(256.0, 320.0);
const RIM: vec2<f32> = vec2<f32>(128.0, 62.0);
const SPRITE_RADIUS: f32 = 64.0;
const TENTACLE_REACH: f32 = 3.75;

fn fade_down(y: f32) -> f32 {
    if (y < 0.4) {
        return mix(1.0, 0.7, y / 0.4);
    }
    if (y < 0.7) {
        return mix(0.7, 0.15, (y - 0.4) / 0.3);
    }
    return mix(0.15, 0.0, min((y - 0.7) / 0.15, 1.0));
}

// How far the bell has closed, from 0 to 1, `p` of the way through a pulse: quick, then slow.
fn contraction(p: f32) -> f32 {
    if (p < 0.28) {
        return sin(p / 0.28 * 1.5707963);
    }
    return 0.5 + 0.5 * cos((p - 0.28) / 0.72 * 3.1415927);
}

// How much of a pulse's push has carried the jellyfish on, `p` of the way through it.
fn carried(p: f32) -> f32 {
    return (1.0 - exp(-3.0 * p)) / (1.0 - exp(-3.0));
}

struct Jelly {
    at: vec2<f32>,
    radius: f32,
    tilt: f32,
    closed: f32,
    hue: f32,
    strength: f32,
    seed: f32,
}

// Where the `j`th jellyfish is: it starts in its own place and, once it has swum out past the
// top, rises again from below at a place its lap picks.
fn jelly(j: u32, size: vec2<f32>, t: f32) -> Jelly {
    var slots = array<vec3<f32>, 5>(
        vec3<f32>(0.2, 0.4, 0.82),
        vec3<f32>(0.8, 0.3, 1.0),
        vec3<f32>(0.4, 0.9, 0.92),
        vec3<f32>(0.66, 0.72, 0.58),
        vec3<f32>(0.07, 0.97, 0.62),
    );
    var hues = array<f32, 5>(0.12, 0.9, 0.3, 0.78, 0.55);
    let slot = slots[j];
    let depth = slot.z;
    let base = clamp(min(size.x, size.y * 1.25) * 0.11, 58.0, 120.0);
    let radius = base * (0.62 + 0.55 * depth) * (0.88 + 0.24 * hash3(j, 1u, 0u));
    let rate = 0.86 + 0.28 * hash3(j, 2u, 0u);
    let wander = hash3(j, 3u, 0u) * 100.0;
    let pulses = t * rate / 2.6 + hash3(j, 4u, 0u);
    let p = fract(pulses);
    let stride = 62.0 * (0.5 + 0.5 * depth) / 1.15;
    let swum = stride * (floor(pulses) + carried(p));
    let below = size.y + 1.2 * radius;
    let lap_length = size.y + 5.6 * radius;
    let gone = below - slot.y * size.y + swum;
    let lap = floor(gone / lap_length);
    var out: Jelly;
    out.radius = radius;
    out.closed = contraction(p);
    out.tilt = sin(t * 0.11 + wander) * 0.5;
    out.strength = 0.54 + 0.56 * depth;
    out.seed = hash3(j, 5u, 0u) * 6.28;
    var x = slot.x;
    out.hue = hues[j];
    if (lap > 0.0) {
        let l = u32(lap);
        x = 0.08 + 0.84 * hash3(j, l, 6u);
        let pick = hash3(j, l, 7u);
        out.hue = select(0.65 + 0.35 * hash3(j, l, 8u), 0.35 * hash3(j, l, 8u), pick < 0.5);
    }
    out.at = vec2<f32>(
        x * size.x + sin(t * 0.07 + wander) * radius * 0.8,
        below - (gone - lap * lap_length),
    );
    return out;
}

// The sprite's light and white core at `p`, in points, for one jellyfish: its bell narrowing
// and lengthening as it closes, its tentacles swaying the more the further down they hang.
fn sample_jelly(p: vec2<f32>, one: Jelly, t: f32, image_rect: vec4<f32>) -> vec2<f32> {
    let d = p - one.at;
    let turn = vec2<f32>(cos(one.tilt), sin(one.tilt));
    var q = vec2<f32>(d.x * turn.x + d.y * turn.y, -d.x * turn.y + d.y * turn.x) / one.radius;
    let hanging = clamp(q.y / TENTACLE_REACH, 0.0, 1.0);
    q.x = q.x / mix(1.0 - 0.17 * one.closed, 1.0, hanging);
    if (q.y < 0.0) {
        q.y = q.y / (1.0 + 0.1 * one.closed);
    }
    q.x = q.x + sin(t * 1.5 - q.y * 1.7 + one.seed) * 0.16 * hanging;
    let texel = RIM + q * SPRITE_RADIUS;
    if (any(texel < vec2<f32>(0.5)) || any(texel > SPRITE - 0.5)) {
        return vec2<f32>(0.0);
    }
    return image_texel(texel + vec2<f32>(0.0, SPRITE_TOP), image_rect).rg * one.strength;
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

    let colours = shader.params[1];
    let accents = shader.params[2];
    var inks: Inks;
    inks.ink = unpack(colours.z);
    inks.ground = unpack(colours.w);
    inks.accents = array<vec3<f32>, 4>(unpack(accents.x), unpack(accents.y), unpack(accents.z), unpack(accents.w));
    inks.sparkles = true;
    let opacity = shader.params[3].x * fade;

    let cell = floor(local / CELL);
    let inside = local - cell * CELL;
    let centre = (cell + 0.5) * CELL;
    var count = 5u;
    if (size.x < 600.0) {
        count = 3u;
    } else if (size.x < 1100.0) {
        count = 4u;
    }
    var first = 0.0;
    var second = 0.0;
    var core = 0.0;
    for (var j = 0u; j < count; j = j + 1u) {
        let one = jelly(j, size, t);
        let reach = one.radius * 5.0;
        if (abs(centre.x - one.at.x) > reach || abs(centre.y - one.at.y) > reach) {
            continue;
        }
        let lit = sample_jelly(centre, one, t, shader.image);
        first = first + (1.0 - one.hue) * lit.x;
        second = second + one.hue * lit.x;
        core = core + lit.y;
    }
    let light = min(1.0, first + second + 0.55 * core);
    if (light >= 0.055) {
        let blend = round(second / (first + second + 0.0001) * 5.0) / 5.0;
        let hue = mix(unpack(colours.x), unpack(colours.y), blend);
        return ascii_out(ascii_cell(inside, cell, light, core, hue, inks, t, scale, shader.image), opacity);
    }

    // A speck falling down its column, stepping a cell at a time.
    var colour = grid(inside, inks.ink, scale);
    let x = u32(cell.x);
    let fall = 3.0 + 8.0 * hash3(x, 11u, 0u);
    let speck = u32(i32(cell.y) - i32(floor(t * fall / CELL.y)) + 100000);
    let chance = hash3(x, speck, 12u);
    if (chance < 1.0 / 80.0) {
        var specks = array<f32, 8>(0.0, 0.0, 0.0, DASH, DASH, 2.0, 1.0, MIDDLE_DOT);
        let which = specks[u32(hash3(x, speck, 13u) * 8.0) & 7u];
        let alpha = (0.14 + 0.3 * hash3(x, speck, 14u)) * glyph_cover(which, inside, shader.image);
        colour = over(vec4<f32>(mix(inks.ink, unpack(colours.x), 0.25) * alpha, alpha), colour);
    }
    return ascii_out(colour, opacity);
}
