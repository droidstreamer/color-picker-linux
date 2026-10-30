// Direct port of the colour maths from colour_math.hpp (originally colorpicker.py).
// f64::round rounds half away from zero, like C++ std::lround, so results match.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rgb {
    pub r: i32,
    pub g: i32,
    pub b: i32,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Hsv {
    pub h: i32,
    pub s: i32,
    pub v: i32,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Hsl {
    pub h: i32,
    pub s: i32,
    pub l: i32,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cmyk {
    pub c: i32,
    pub m: i32,
    pub y: i32,
    pub k: i32,
}
#[derive(Clone, Copy)]
struct Lab {
    l: f64,
    a: f64,
    b: f64,
}

pub const fn rgb(r: i32, g: i32, b: i32) -> Rgb {
    Rgb { r, g, b }
}

fn ri(x: f64) -> i32 {
    x.round() as i32
}

fn srgb_to_linear(c: i32) -> f64 {
    let c = c as f64 / 255.0;
    if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
}

fn lab_f(t: f64) -> f64 {
    if t > 0.008856 { t.cbrt() } else { 7.787 * t + 16.0 / 116.0 }
}

fn rgb_to_lab(c: Rgb) -> Lab {
    let (rl, gl, bl) = (srgb_to_linear(c.r), srgb_to_linear(c.g), srgb_to_linear(c.b));
    let x = (rl * 0.4124564 + gl * 0.3575761 + bl * 0.1804375) / 0.95047;
    let y = rl * 0.2126729 + gl * 0.7151522 + bl * 0.0721750;
    let z = (rl * 0.0193339 + gl * 0.1191920 + bl * 0.9503041) / 1.08883;
    let (fx, fy, fz) = (lab_f(x), lab_f(y), lab_f(z));
    Lab { l: 116.0 * fy - 16.0, a: 500.0 * (fx - fy), b: 200.0 * (fy - fz) }
}

fn cie94(a: &Lab, b: &Lab) -> f64 {
    let dl = a.l - b.l;
    let c1 = a.a.hypot(a.b);
    let c2 = b.a.hypot(b.b);
    let dc = c1 - c2;
    let (da, db) = (a.a - b.a, a.b - b.b);
    let dh2 = da * da + db * db - dc * dc;
    let dh = if dh2 > 0.0 { dh2.sqrt() } else { 0.0 };
    let sc = 1.0 + 0.045 * c1;
    let sh = 1.0 + 0.015 * c1;
    (dl * dl + (dc / sc).powi(2) + (dh / sh).powi(2)).sqrt()
}

fn unit(c: Rgb) -> (f64, f64, f64) {
    (c.r as f64 / 255.0, c.g as f64 / 255.0, c.b as f64 / 255.0)
}

pub fn rgb_to_hsv(c: Rgb) -> Hsv {
    let (r, g, b) = unit(c);
    let mx = r.max(g).max(b);
    let mn = r.min(g).min(b);
    let d = mx - mn;
    let s = if mx == 0.0 { 0.0 } else { d / mx };
    let h = if d == 0.0 {
        0.0
    } else if mx == r {
        ((g - b) / d).rem_euclid(6.0)
    } else if mx == g {
        (b - r) / d + 2.0
    } else {
        (r - g) / d + 4.0
    };
    Hsv { h: ri(h * 60.0).rem_euclid(360), s: ri(s * 100.0), v: ri(mx * 100.0) }
}

pub fn hsv_to_rgb(h: f64, s: f64, v: f64) -> Rgb {
    let h = h.rem_euclid(360.0);
    let s = (s / 100.0).clamp(0.0, 1.0);
    let v = (v / 100.0).clamp(0.0, 1.0);
    let c = v * s;
    let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
    let m = v - c;
    let (rp, gp, bp) = match h {
        h if h < 60.0 => (c, x, 0.0),
        h if h < 120.0 => (x, c, 0.0),
        h if h < 180.0 => (0.0, c, x),
        h if h < 240.0 => (0.0, x, c),
        h if h < 300.0 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    rgb(ri((rp + m) * 255.0), ri((gp + m) * 255.0), ri((bp + m) * 255.0))
}

pub fn rgb_to_hsl(c: Rgb) -> Hsl {
    let (r, g, b) = unit(c);
    let mx = r.max(g).max(b);
    let mn = r.min(g).min(b);
    let l = (mx + mn) / 2.0;
    if mx == mn {
        return Hsl { h: 0, s: 0, l: ri(l * 100.0) };
    }
    let d = mx - mn;
    let s = if l > 0.5 { d / (2.0 - mx - mn) } else { d / (mx + mn) };
    let h = if mx == r {
        (g - b) / d + if g < b { 6.0 } else { 0.0 }
    } else if mx == g {
        (b - r) / d + 2.0
    } else {
        (r - g) / d + 4.0
    };
    Hsl { h: ri(h / 6.0 * 360.0), s: ri(s * 100.0), l: ri(l * 100.0) }
}

// Screen CMYK - a straightforward derivation from RGB, not a print/ICC profile.
pub fn rgb_to_cmyk(c: Rgb) -> Cmyk {
    let (r, g, b) = unit(c);
    let k = 1.0 - r.max(g).max(b);
    let (c, m, y) = if k >= 1.0 {
        (0.0, 0.0, 0.0)
    } else {
        ((1.0 - r - k) / (1.0 - k), (1.0 - g - k) / (1.0 - k), (1.0 - b - k) / (1.0 - k))
    };
    Cmyk { c: ri(c * 100.0), m: ri(m * 100.0), y: ri(y * 100.0), k: ri(k * 100.0) }
}

fn cmyk_to_rgb(c: f64, m: f64, y: f64, k: f64) -> Rgb {
    let [c, m, y, k] = [c, m, y, k].map(|v| v.clamp(0.0, 100.0) / 100.0);
    let ch = |v: f64| ri(255.0 * (1.0 - v) * (1.0 - k)).clamp(0, 255);
    rgb(ch(c), ch(m), ch(y))
}

fn clamp8(n: f64) -> i32 {
    n.round().clamp(0.0, 255.0) as i32
}

pub fn parse_hex(text: &str) -> Option<Rgb> {
    let t: String = text.chars().filter(|c| !c.is_whitespace()).collect();
    let t = t.strip_prefix('#').unwrap_or(&t);
    let t = if t.len() == 3 { t.chars().flat_map(|c| [c, c]).collect() } else { t.to_string() };
    if t.len() != 6 || !t.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let ch = |i: usize| i32::from_str_radix(&t[i..i + 2], 16).unwrap();
    Some(rgb(ch(0), ch(2), ch(4)))
}

// Every number in the text, like the regex -?\d+\.?\d*
fn extract_nums(text: &str) -> Vec<f64> {
    let b = text.as_bytes();
    let digits = |mut i: usize| {
        while i < b.len() && b[i].is_ascii_digit() {
            i += 1;
        }
        i
    };
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        let neg = b[i] == b'-' && b.get(i + 1).is_some_and(u8::is_ascii_digit);
        if !neg && !b[i].is_ascii_digit() {
            i += 1;
            continue;
        }
        let start = i;
        i = digits(i + neg as usize);
        if i < b.len() && b[i] == b'.' {
            i = digits(i + 1);
        }
        out.push(text[start..i].parse().unwrap());
    }
    out
}

pub fn parse_rgb(text: &str) -> Option<Rgb> {
    let n = extract_nums(text);
    (n.len() >= 3).then(|| rgb(clamp8(n[0]), clamp8(n[1]), clamp8(n[2])))
}

pub fn parse_hsl(text: &str) -> Option<Rgb> {
    let n = extract_nums(text);
    if n.len() < 3 {
        return None;
    }
    let h = (n[0].round() as i64).rem_euclid(360) as f64;
    let s = n[1].round().clamp(0.0, 100.0) / 100.0;
    let l = n[2].round().clamp(0.0, 100.0) / 100.0;
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
    let m = l - c / 2.0;
    let seg = [[c, x, 0.0], [x, c, 0.0], [0.0, c, x], [0.0, x, c], [x, 0.0, c], [c, 0.0, x]];
    let p = seg[(h as usize / 60) % 6];
    Some(rgb(clamp8((p[0] + m) * 255.0), clamp8((p[1] + m) * 255.0), clamp8((p[2] + m) * 255.0)))
}

pub fn parse_cmyk(text: &str) -> Option<Rgb> {
    let n = extract_nums(text);
    (n.len() >= 4).then(|| cmyk_to_rgb(n[0], n[1], n[2], n[3]))
}

pub fn luminance(c: Rgb) -> f64 {
    0.2126 * srgb_to_linear(c.r) + 0.7152 * srgb_to_linear(c.g) + 0.0722 * srgb_to_linear(c.b)
}

fn rgb_to_l(c: Rgb) -> f64 {
    let y = 0.2126729 * srgb_to_linear(c.r) + 0.7151522 * srgb_to_linear(c.g) + 0.0721750 * srgb_to_linear(c.b);
    116.0 * lab_f(y) - 16.0
}

fn blend(a: Rgb, b: Rgb, t: f64) -> Rgb {
    let mix = |x: i32, y: i32| clamp8(x as f64 * (1.0 - t) + y as f64 * t);
    rgb(mix(a.r, b.r), mix(a.g, b.g), mix(a.b, b.b))
}

fn t_for_l(c: Rgb, target: f64, direction: Rgb) -> f64 {
    let increasing = rgb_to_l(direction) > rgb_to_l(c);
    let (mut lo, mut hi) = (0.0, 1.0);
    for _ in 0..60 {
        let mid = (lo + hi) / 2.0;
        let below = rgb_to_l(blend(c, direction, mid)) < target;
        if below == increasing { lo = mid } else { hi = mid }
    }
    (lo + hi) / 2.0
}

// Returns 9 colours: 4 tints, the current colour, 4 shades.
pub fn perceptual_shade_row(c: Rgb) -> Vec<Rgb> {
    const N: i32 = 4;
    const WHITE: Rgb = rgb(255, 255, 255);
    const BLACK: Rgb = rgb(0, 0, 0);
    let l = rgb_to_l(c);
    let step_up = (96.0f64.min(l + (96.0 - l)) - l) / (N + 1) as f64;
    let step_dn = l / (N + 1) as f64;
    let tints = (1..=N).rev().map(|i| {
        let target = 99.0f64.min(l + i as f64 * step_up);
        blend(c, WHITE, t_for_l(c, target, WHITE))
    });
    let shades = (1..=N).map(|i| {
        let target = 0.5f64.max(l - i as f64 * step_dn);
        blend(c, BLACK, t_for_l(c, target, BLACK))
    });
    tints.chain([c]).chain(shades).collect()
}

// ---------------------------------------------------------------------------
// Colour model: xkcd 947-colour survey, nearest CIE94 match
// ---------------------------------------------------------------------------
const XKCD_TABLE: &[(&str, u32)] = &include!("xkcd_data.rs");

pub struct ColourModel {
    labs: Vec<Lab>,
}

impl ColourModel {
    pub fn new() -> Self {
        let labs = XKCD_TABLE
            .iter()
            .map(|&(_, hex)| rgb_to_lab(rgb((hex >> 16) as i32 & 0xFF, (hex >> 8) as i32 & 0xFF, hex as i32 & 0xFF)))
            .collect();
        ColourModel { labs }
    }

    pub fn nearest_name(&self, c: Rgb) -> &'static str {
        let target = rgb_to_lab(c);
        let mut best = (0, f64::INFINITY);
        for (i, lab) in self.labs.iter().enumerate() {
            let d = cie94(&target, lab);
            if d < best.1 {
                best = (i, d);
            }
        }
        XKCD_TABLE[best.0].0
    }
}

// Title-case, like Python's str.title() for these space-separated names.
pub fn title_case(s: &str) -> String {
    let mut start = true;
    s.chars()
        .map(|c| {
            if c.is_whitespace() {
                start = true;
                c
            } else {
                let out = if start { c.to_ascii_uppercase() } else { c.to_ascii_lowercase() };
                start = false;
                out
            }
        })
        .collect()
}

pub fn hex(c: Rgb) -> String {
    format!("#{:02X}{:02X}{:02X}", c.r, c.g, c.b)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Expected values come from the C++ version (identical on a 4,913-colour grid at the port).
    #[test]
    fn conversions() {
        let c = rgb(52, 152, 219);
        assert_eq!(rgb_to_hsv(c), Hsv { h: 204, s: 76, v: 86 });
        assert_eq!(rgb_to_hsl(c), Hsl { h: 204, s: 70, l: 53 });
        assert_eq!(rgb_to_cmyk(c), Cmyk { c: 76, m: 31, y: 0, k: 14 });
        assert_eq!(rgb_to_cmyk(rgb(0, 0, 0)), Cmyk { c: 0, m: 0, y: 0, k: 100 });
        assert_eq!(hsv_to_rgb(204.0, 76.0, 86.0), rgb(53, 153, 219));
        assert_eq!(hsv_to_rgb(360.0, 100.0, 100.0), rgb(255, 0, 0));
        assert_eq!(hex(c), "#3498DB");
    }

    #[test]
    fn parsers() {
        assert_eq!(parse_hex(" #3498db "), Some(rgb(52, 152, 219)));
        assert_eq!(parse_hex("f0a"), Some(rgb(255, 0, 170)));
        assert_eq!(parse_hex("#12345"), None);
        assert_eq!(parse_hex("#12345g"), None);
        assert_eq!(parse_rgb("rgb(52, 152, 300)"), Some(rgb(52, 152, 255)));
        assert_eq!(parse_rgb("rgb(-5, 1.6, 2.)"), Some(rgb(0, 2, 2)));
        assert_eq!(parse_rgb("rgb(1, 2)"), None);
        assert_eq!(parse_hsl("hsl(204, 70%, 53%)"), Some(rgb(51, 152, 219)));
        assert_eq!(parse_hsl("hsl(-120, 100%, 50%)"), Some(rgb(0, 0, 255)));
        assert_eq!(parse_cmyk("cmyk(76%, 31%, 0%, 14%)"), Some(rgb(53, 151, 219)));
        assert_eq!(parse_cmyk("cmyk(1, 2, 3)"), None);
        assert_eq!(extract_nums("a-b -3 4.5.6"), vec![-3.0, 4.5, 6.0]);
    }

    #[test]
    fn names_and_shades() {
        let m = ColourModel::new();
        assert_eq!(XKCD_TABLE.len(), 1082);
        assert_eq!(m.nearest_name(rgb(0x03, 0x43, 0xDF)), "blue");
        assert_eq!(m.nearest_name(rgb(0xF0, 0xF8, 0xFF)), "Alice Blue");
        assert_eq!(title_case("cloudy blue"), "Cloudy Blue");
        let row = perceptual_shade_row(rgb(52, 152, 219));
        assert_eq!(row.len(), 9);
        assert_eq!(row[4], rgb(52, 152, 219));
        assert!(rgb_to_l(row[0]) > rgb_to_l(row[3]) && rgb_to_l(row[5]) > rgb_to_l(row[8]));
    }
}

