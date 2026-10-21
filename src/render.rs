//! Cartoon rendering layer.
//!
//! The look is deliberately "modern cartoon": chunky ink outlines, sticker
//! panels with hard drop shadows, a halftone sunset backdrop, comic bursts and
//! a mascot. All artwork is drawn procedurally with macroquad primitives; the
//! only asset is the bundled display font.

use macroquad::prelude::*;

use crate::game::{Game, Phase, PieceKind};

pub const WINDOW_W: f32 = 1280.0;
pub const WINDOW_H: f32 = 800.0;

pub const CELL: f32 = 32.0;
pub const BOARD_X: f32 = 480.0;
pub const BOARD_Y: f32 = 112.0;

// ---------------------------------------------------------------------------
// Palette
// ---------------------------------------------------------------------------

const fn rgb(hex: u32) -> Color {
    Color::new(
        ((hex >> 16) & 0xFF) as f32 / 255.0,
        ((hex >> 8) & 0xFF) as f32 / 255.0,
        (hex & 0xFF) as f32 / 255.0,
        1.0,
    )
}

const INK: Color = rgb(0x140B28);
const PAPER: Color = rgb(0xFFF1D6);
const PAPER_DARK: Color = rgb(0xE4C99F);
const WELL: Color = rgb(0x1A0F33);
const CREAM: Color = rgb(0xFFE8B8);
const YELLOW: Color = rgb(0xFFD23F);
const CYAN: Color = rgb(0x37E6FF);
const PINK: Color = rgb(0xFF3F8E);
const GREEN: Color = rgb(0x57E86B);
const RED: Color = rgb(0xFF4D5E);
const PURPLE: Color = rgb(0xC05CFF);
const ORANGE: Color = rgb(0xFF9E2C);
const BLUE: Color = rgb(0x4D7BFF);

fn block_color(id: u8) -> Color {
    match id {
        1 => rgb(0x2BD9FE),
        2 => YELLOW,
        3 => PURPLE,
        4 => GREEN,
        5 => RED,
        6 => BLUE,
        7 => ORANGE,
        _ => CREAM,
    }
}

pub fn party_colors() -> [Color; 4] {
    [YELLOW, CYAN, PINK, GREEN]
}

pub fn dust_color() -> Color {
    CREAM
}

fn kind_color(kind: PieceKind) -> Color {
    block_color(kind.id())
}

fn alpha(color: Color, a: f32) -> Color {
    Color::new(color.r, color.g, color.b, color.a * a)
}

fn blend(a: Color, b: Color, t: f32) -> Color {
    Color::new(
        a.r + (b.r - a.r) * t,
        a.g + (b.g - a.g) * t,
        a.b + (b.b - a.b) * t,
        a.a + (b.a - a.a) * t,
    )
}

fn shade(color: Color, factor: f32) -> Color {
    Color::new(
        (color.r * factor).clamp(0.0, 1.0),
        (color.g * factor).clamp(0.0, 1.0),
        (color.b * factor).clamp(0.0, 1.0),
        color.a,
    )
}

// ---------------------------------------------------------------------------
// Assets
// ---------------------------------------------------------------------------

pub struct Assets {
    pub font: Font,
    pub background: Texture2D,
    pub overlay: Texture2D,
}

impl Assets {
    pub fn load() -> Assets {
        let font = load_ttf_font_from_bytes(include_bytes!("../assets/LilitaOne-Regular.ttf"))
            .expect("bundled font must load");
        Assets {
            font,
            background: build_background(),
            overlay: build_overlay(),
        }
    }
}

fn hash2(x: u32, y: u32, seed: u32) -> u32 {
    let mut h = x
        .wrapping_mul(374_761_393)
        .wrapping_add(y.wrapping_mul(668_265_263))
        .wrapping_add(seed);
    h = (h ^ (h >> 13)).wrapping_mul(1_274_126_177);
    h ^ (h >> 16)
}

fn build_background() -> Texture2D {
    let w = WINDOW_W as u32;
    let h = WINDOW_H as u32;
    let mut img = Image::gen_image_color(w as u16, h as u16, INK);
    let cx = 640.0f32;
    let cy = 500.0f32;
    let sun_r = 300.0f32;

    for y in 0..h {
        for x in 0..w {
            let fx = x as f32;
            let fy = y as f32;
            let dx = fx - cx;
            let dy = fy - cy;
            let dist = (dx * dx + dy * dy).sqrt();
            let mut color = blend(rgb(0x2B1652), rgb(0x120A24), fy / h as f32);

            // Sunburst rays radiating from behind the playfield.
            let angle = dy.atan2(dx);
            let sector = (((angle + std::f32::consts::PI) / (std::f32::consts::PI / 9.0)).floor()
                as i32)
                .rem_euclid(2);
            if sector == 0 && dist > sun_r * 0.8 {
                color = blend(color, rgb(0x3E2173), 0.42);
            }

            // Retro sun disc with horizontal cut-outs.
            if dist < sun_r {
                let k = ((fy - (cy - sun_r)) / (2.0 * sun_r)).clamp(0.0, 1.0);
                let mut sun = blend(rgb(0xFFC24A), rgb(0xFF3F8E), k);
                if fy > cy + 8.0 && ((fy - cy) as i32 % 38) < 13 {
                    sun = color;
                }
                color = blend(sun, color, (dist / sun_r).powf(6.0) * 0.35);
                if (dist - sun_r).abs() < 3.0 {
                    color = INK;
                }
            }

            // Vignette.
            let vx = (fx / w as f32 - 0.5).abs();
            let vy = (fy / h as f32 - 0.5).abs();
            let vig = (vx.max(vy) - 0.30).max(0.0) * 0.9;
            color = blend(color, INK, vig.min(0.55));

            img.set_pixel(x, y, color);
        }
    }

    // Halftone dots.
    for y in (0..h).step_by(14) {
        for x in (0..w).step_by(14) {
            let ox = x + ((y / 14) % 2) * 7;
            if ox >= w {
                continue;
            }
            if !hash2(ox / 14, y / 14, 7).is_multiple_of(3) {
                continue;
            }
            let dot = blend(img.get_pixel(ox, y), CREAM, 0.16);
            for dy in 0..3u32 {
                for dx in 0..3u32 {
                    let px = (ox + dx).min(w - 1);
                    let py = (y + dy).min(h - 1);
                    img.set_pixel(px, py, dot);
                }
            }
        }
    }

    // Sparkles.
    for i in 0..120u32 {
        let x = hash2(i, 1, 13) % w;
        let y = hash2(i, 2, 29) % h;
        let star = blend(img.get_pixel(x, y), CREAM, 0.75);
        for d in 0..3u32 {
            if x + d < w {
                img.set_pixel(x + d, y, star);
            }
            if y + d < h {
                img.set_pixel(x, y + d, star);
            }
        }
    }

    Texture2D::from_image(&img)
}

fn build_overlay() -> Texture2D {
    let w = WINDOW_W as u32;
    let h = WINDOW_H as u32;
    let mut img = Image::gen_image_color(w as u16, h as u16, Color::new(0.0, 0.0, 0.0, 0.0));
    for y in 0..h {
        for x in 0..w {
            let dx = (x as f32 / w as f32 - 0.5).abs();
            let dy = (y as f32 / h as f32 - 0.5).abs();
            let d = (dx * dx + dy * dy).sqrt();
            let mut a = ((d - 0.42).max(0.0) * 0.85).min(0.5);
            if y % 4 < 2 {
                a += 0.03;
            }
            img.set_pixel(x, y, Color::new(0.0, 0.0, 0.0, a.min(0.62)));
        }
    }
    Texture2D::from_image(&img)
}

// ---------------------------------------------------------------------------
// Scene description
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Screen {
    Menu,
    Playing,
    Paused,
    GameOver,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mood {
    Idle,
    Happy,
    Worried,
    Sad,
    Excited,
}

pub struct Toast {
    pub text: String,
    pub color: Color,
    pub timer: f32,
    pub total: f32,
}

impl Toast {
    pub fn new(text: &str, color: Color) -> Toast {
        Toast {
            text: text.to_string(),
            color,
            timer: 1.6,
            total: 1.6,
        }
    }

    pub fn update(&mut self, dt: f32) {
        self.timer = (self.timer - dt).max(0.0);
    }

    pub fn active(&self) -> bool {
        self.timer > 0.0
    }
}

pub struct Particle {
    pub pos: Vec2,
    pub vel: Vec2,
    pub life: f32,
    pub max_life: f32,
    pub color: Color,
    pub size: f32,
    pub rot: f32,
    pub spin: f32,
    pub gravity: f32,
}

#[derive(Default)]
pub struct Particles {
    items: Vec<Particle>,
    seed: u64,
}

impl Particles {
    pub fn clear(&mut self) {
        self.items.clear();
    }

    pub fn new() -> Particles {
        Particles {
            items: Vec::new(),
            seed: 0x5EED_1234_ABCD,
        }
    }

    fn rand(&mut self) -> f32 {
        self.seed ^= self.seed << 13;
        self.seed ^= self.seed >> 7;
        self.seed ^= self.seed << 17;
        ((self.seed >> 40) as f32) / (1u64 << 24) as f32
    }

    pub fn update(&mut self, dt: f32) {
        for p in self.items.iter_mut() {
            p.life -= dt;
            p.vel.y += p.gravity * dt;
            p.pos += p.vel * dt;
            p.rot += p.spin * dt;
        }
        self.items.retain(|p| p.life > 0.0);
    }

    pub fn dust(&mut self, x: f32, y: f32, count: usize, color: Color) {
        for _ in 0..count {
            let life = 0.25 + self.rand() * 0.35;
            let px = x + (self.rand() - 0.5) * 26.0;
            let vx = (self.rand() - 0.5) * 220.0;
            let vy = -self.rand() * 120.0;
            let size = 4.0 + self.rand() * 4.0;
            let rot = self.rand() * std::f32::consts::TAU;
            let spin = (self.rand() - 0.5) * 8.0;
            self.items.push(Particle {
                pos: vec2(px, y),
                vel: vec2(vx, vy),
                life,
                max_life: life,
                color: alpha(color, 0.85),
                size,
                rot,
                spin,
                gravity: 520.0,
            });
        }
    }

    pub fn confetti(&mut self, x: f32, y: f32, count: usize, palette: &[Color]) {
        for i in 0..count {
            let life = 0.6 + self.rand() * 0.8;
            let px = x + (self.rand() - 0.5) * 260.0;
            let vx = (self.rand() - 0.5) * 340.0;
            let vy = -120.0 - self.rand() * 260.0;
            let size = 5.0 + self.rand() * 6.0;
            let rot = self.rand() * std::f32::consts::TAU;
            let spin = (self.rand() - 0.5) * 14.0;
            self.items.push(Particle {
                pos: vec2(px, y),
                vel: vec2(vx, vy),
                life,
                max_life: life,
                color: alpha(palette[i % palette.len()], 0.95),
                size,
                rot,
                spin,
                gravity: 640.0,
            });
        }
    }

    pub fn sparkle(&mut self, x: f32, y: f32, count: usize) {
        let palette = [YELLOW, CYAN, PINK, CREAM];
        for i in 0..count {
            let life = 0.3 + self.rand() * 0.4;
            let px = x + (self.rand() - 0.5) * 300.0;
            let py = y + (self.rand() - 0.5) * 20.0;
            let vx = (self.rand() - 0.5) * 120.0;
            let vy = -self.rand() * 180.0;
            let size = 3.0 + self.rand() * 4.0;
            let rot = self.rand() * std::f32::consts::TAU;
            let spin = (self.rand() - 0.5) * 12.0;
            self.items.push(Particle {
                pos: vec2(px, py),
                vel: vec2(vx, vy),
                life,
                max_life: life,
                color: palette[i % palette.len()],
                size,
                rot,
                spin,
                gravity: 300.0,
            });
        }
    }

    fn draw(&self) {
        for p in &self.items {
            let a = (p.life / p.max_life).clamp(0.0, 1.0);
            draw_rectangle_ex(
                p.pos.x,
                p.pos.y,
                p.size,
                p.size * 0.7,
                DrawRectangleParams {
                    offset: vec2(0.5, 0.5),
                    rotation: p.rot,
                    color: alpha(p.color, a),
                },
            );
        }
    }
}

pub struct Scene<'a> {
    pub assets: &'a Assets,
    pub screen: Screen,
    pub game: &'a Game,
    pub high_score: u64,
    pub start_level: u32,
    pub music_on: bool,
    pub toast: Option<&'a Toast>,
    pub mood: Mood,
    pub particles: &'a Particles,
    pub shake: Vec2,
    pub flash: f32,
    pub t: f32,
}

// ---------------------------------------------------------------------------
// Low-level cartoon helpers
// ---------------------------------------------------------------------------

fn rounded_rect(x: f32, y: f32, w: f32, h: f32, r: f32, color: Color) {
    let r = r.min(w * 0.5).min(h * 0.5);
    if r <= 0.0 {
        draw_rectangle(x, y, w, h, color);
        return;
    }
    draw_rectangle(x + r, y, w - 2.0 * r, h, color);
    draw_rectangle(x, y + r, w, h - 2.0 * r, color);
    draw_circle(x + r, y + r, r, color);
    draw_circle(x + w - r, y + r, r, color);
    draw_circle(x + r, y + h - r, r, color);
    draw_circle(x + w - r, y + h - r, r, color);
}

fn ink_rect(x: f32, y: f32, w: f32, h: f32, r: f32, thickness: f32, fill: Color) {
    rounded_rect(
        x - thickness,
        y - thickness,
        w + 2.0 * thickness,
        h + 2.0 * thickness,
        r + thickness,
        INK,
    );
    rounded_rect(x, y, w, h, r, fill);
}

fn sticker(x: f32, y: f32, w: f32, h: f32, fill: Color) {
    rounded_rect(x + 8.0, y + 10.0, w, h, 18.0, alpha(INK, 0.45));
    ink_rect(x, y, w, h, 16.0, 5.0, fill);
    rounded_rect(
        x + 14.0,
        y + 12.0,
        w - 28.0,
        5.0,
        2.5,
        Color::new(1.0, 1.0, 1.0, 0.35),
    );
    draw_rectangle_lines(
        x + 16.0,
        y + 16.0,
        w - 32.0,
        h - 32.0,
        2.0,
        alpha(INK, 0.16),
    );
    for (bx, by) in [
        (x + 18.0, y + 18.0),
        (x + w - 18.0, y + 18.0),
        (x + 18.0, y + h - 18.0),
        (x + w - 18.0, y + h - 18.0),
    ] {
        draw_circle(bx, by, 4.0, INK);
        draw_circle(bx - 0.6, by - 0.6, 1.6, alpha(CREAM, 0.6));
    }
}

fn text_at(text: &str, x: f32, y: f32, size: f32, color: Color, font: &Font, rot: f32) {
    draw_text_ex(
        text,
        x,
        y,
        TextParams {
            font: Some(font),
            font_size: size as u16,
            font_scale: 1.0,
            font_scale_aspect: 1.0,
            rotation: rot,
            color,
        },
    );
}

#[allow(clippy::too_many_arguments)]
fn text_outlined(
    text: &str,
    x: f32,
    y: f32,
    size: f32,
    fill: Color,
    font: &Font,
    thickness: f32,
    rot: f32,
) {
    let offsets = [
        (-thickness, 0.0),
        (thickness, 0.0),
        (0.0, -thickness),
        (0.0, thickness),
        (-thickness, -thickness),
        (thickness, -thickness),
        (-thickness, thickness),
        (thickness, thickness),
    ];
    for (dx, dy) in offsets {
        text_at(text, x + dx, y + dy, size, INK, font, rot);
    }
    text_at(text, x, y, size, fill, font, rot);
}

fn text_center(text: &str, cx: f32, cy: f32, size: f32, fill: Color, font: &Font, thickness: f32) {
    let dims = measure_text(text, Some(font), size as u16, 1.0);
    let x = cx - dims.width / 2.0;
    let y = cy + dims.height * 0.36;
    text_outlined(text, x, y, size, fill, font, thickness, 0.0);
}

fn tab_label(x: f32, y: f32, text: &str, fill: Color, font: &Font) -> f32 {
    let dims = measure_text(text, Some(font), 22, 1.0);
    let w = dims.width + 26.0;
    rounded_rect(x + 4.0, y + 5.0, w, 30.0, 9.0, alpha(INK, 0.4));
    ink_rect(x, y, w, 30.0, 8.0, 3.0, fill);
    text_center(text, x + w / 2.0, y + 15.0, 22.0, INK, font, 0.0);
    w
}

fn draw_block(x: f32, y: f32, size: f32, id: u8, opacity: f32) {
    let base = block_color(id);
    rounded_rect(
        x + 2.0,
        y + 3.0,
        size - 2.0,
        size - 2.0,
        6.0,
        alpha(INK, 0.38 * opacity),
    );
    ink_rect(
        x + 1.0,
        y + 1.0,
        size - 2.0,
        size - 2.0,
        5.0,
        2.6,
        alpha(base, opacity),
    );
    rounded_rect(
        x + 5.0,
        y + 5.0,
        size - 10.0,
        4.0,
        2.0,
        alpha(blend(base, Color::new(1.0, 1.0, 1.0, 1.0), 0.55), opacity),
    );
    rounded_rect(
        x + 5.0,
        y + size - 9.0,
        size - 10.0,
        4.0,
        2.0,
        alpha(shade(base, 0.62), opacity),
    );
    rounded_rect(
        x + 6.0,
        y + 6.0,
        size * 0.32,
        3.0,
        1.5,
        alpha(CREAM, 0.8 * opacity),
    );
}

fn draw_ghost_block(x: f32, y: f32, size: f32, id: u8) {
    let base = block_color(id);
    rounded_rect(
        x + 3.0,
        y + 3.0,
        size - 6.0,
        size - 6.0,
        5.0,
        alpha(base, 0.16),
    );
    draw_rectangle_lines(
        x + 4.0,
        y + 4.0,
        size - 8.0,
        size - 8.0,
        2.0,
        alpha(base, 0.55),
    );
}

fn piece_bounds(kind: PieceKind, rot: u8) -> (i32, i32, i32, i32) {
    let cells = crate::game::shape(kind, rot);
    let mut min_x = i32::MAX;
    let mut min_y = i32::MAX;
    let mut max_x = i32::MIN;
    let mut max_y = i32::MIN;
    for &(x, y) in cells.iter() {
        min_x = min_x.min(x);
        min_y = min_y.min(y);
        max_x = max_x.max(x);
        max_y = max_y.max(y);
    }
    (min_x, min_y, max_x, max_y)
}

fn draw_preview(kind: PieceKind, cx: f32, cy: f32, cell: f32) {
    let (min_x, min_y, max_x, max_y) = piece_bounds(kind, 0);
    let w = (max_x - min_x + 1) as f32 * cell;
    let h = (max_y - min_y + 1) as f32 * cell;
    let ox = cx - w / 2.0 - min_x as f32 * cell;
    let oy = cy - h / 2.0 - min_y as f32 * cell;
    for &(x, y) in crate::game::shape(kind, 0).iter() {
        draw_block(
            ox + x as f32 * cell,
            oy + y as f32 * cell,
            cell,
            kind.id(),
            1.0,
        );
    }
}

fn draw_piece_rotated(kind: PieceKind, cx: f32, cy: f32, cell: f32, angle: f32, opacity: f32) {
    let (min_x, min_y, max_x, max_y) = piece_bounds(kind, 0);
    let w = (max_x - min_x + 1) as f32 * cell;
    let h = (max_y - min_y + 1) as f32 * cell;
    let base_x = cx - w / 2.0;
    let base_y = cy - h / 2.0;
    for &(x, y) in crate::game::shape(kind, 0).iter() {
        draw_rectangle_ex(
            base_x + x as f32 * cell + cell / 2.0,
            base_y + y as f32 * cell + cell / 2.0,
            cell,
            cell,
            DrawRectangleParams {
                offset: vec2(0.5, 0.5),
                rotation: angle,
                color: alpha(block_color(kind.id()), opacity),
            },
        );
    }
}

fn star(cx: f32, cy: f32, radius: f32, points: u8, rot: f32, color: Color) {
    let n = points as usize * 2;
    let mut pts = Vec::with_capacity(n);
    for i in 0..n {
        let a = rot + i as f32 / n as f32 * std::f32::consts::TAU;
        let r = if i % 2 == 0 { radius } else { radius * 0.52 };
        pts.push(vec2(cx + a.cos() * r, cy + a.sin() * r));
    }
    for i in 0..n {
        let p1 = pts[i];
        let p2 = pts[(i + 1) % n];
        draw_triangle(vec2(cx, cy), p1, p2, color);
    }
}

fn burst(cx: f32, cy: f32, radius: f32, fill: Color, text: &str, font: &Font, t: f32) {
    let rot = (t * 1.7).sin() * 0.08;
    star(cx, cy, radius + 6.0, 12, rot, INK);
    star(cx, cy, radius, 12, rot, fill);
    star(cx, cy, radius * 0.66, 12, rot + 0.2, alpha(CREAM, 0.25));
    text_center(text, cx, cy - 2.0, radius * 0.32, CREAM, font, 4.0);
}

fn speech_bubble(x: f32, y: f32, text: &str, font: &Font) {
    let dims = measure_text(text, Some(font), 24, 1.0);
    let w = dims.width + 40.0;
    let h = 56.0;
    let bx = x - w / 2.0;
    let by = y - h;
    rounded_rect(bx + 5.0, by + 7.0, w, h, 16.0, alpha(INK, 0.4));
    ink_rect(bx, by, w, h, 14.0, 4.0, PAPER);
    // Tail.
    let tail = vec2(x, y + 26.0);
    draw_triangle(
        vec2(x - 16.0, by + h - 4.0),
        vec2(x + 16.0, by + h - 4.0),
        tail,
        INK,
    );
    draw_triangle(
        vec2(x - 11.0, by + h - 6.0),
        vec2(x + 11.0, by + h - 6.0),
        vec2(x, y + 20.0),
        PAPER,
    );
    text_center(text, x, by + h / 2.0, 24.0, INK, font, 0.0);
}

// ---------------------------------------------------------------------------
// Mascot
// ---------------------------------------------------------------------------

fn arc(cx: f32, cy: f32, r: f32, from: f32, to: f32, thickness: f32, color: Color) {
    let steps = 16;
    let mut prev = vec2(cx + r * from.cos(), cy + r * from.sin());
    for i in 1..=steps {
        let a = from + (to - from) * i as f32 / steps as f32;
        let p = vec2(cx + r * a.cos(), cy + r * a.sin());
        draw_line(prev.x, prev.y, p.x, p.y, thickness, color);
        prev = p;
    }
}

fn draw_mascot(x: f32, y: f32, scale: f32, mood: Mood, t: f32, wave: bool) {
    let bob = (t * 2.6).sin() * 4.0 * scale;
    let cy = y + bob;
    let body = 118.0 * scale;

    draw_ellipse(
        x,
        y + 74.0 * scale,
        58.0 * scale,
        12.0 * scale,
        0.0,
        alpha(INK, 0.35),
    );

    // Feet.
    for side in [-1.0f32, 1.0] {
        rounded_rect(
            x + side * 30.0 * scale - 16.0 * scale,
            cy + 52.0 * scale,
            32.0 * scale,
            20.0 * scale,
            9.0 * scale,
            INK,
        );
        rounded_rect(
            x + side * 30.0 * scale - 12.0 * scale,
            cy + 54.0 * scale,
            24.0 * scale,
            14.0 * scale,
            7.0 * scale,
            PAPER_DARK,
        );
    }

    // Arms.
    let arm_y = cy + 4.0 * scale;
    let left_hand = vec2(
        x - 66.0 * scale,
        arm_y + if wave { -34.0 * scale } else { 20.0 * scale },
    );
    let right_hand = vec2(x + 66.0 * scale, arm_y + 20.0 * scale);
    for hand in [left_hand, right_hand] {
        draw_line(x, arm_y, hand.x, hand.y, 12.0 * scale, INK);
        draw_line(x, arm_y, hand.x, hand.y, 7.0 * scale, YELLOW);
        draw_circle(hand.x, hand.y, 11.0 * scale, INK);
        draw_circle(hand.x, hand.y, 8.0 * scale, CREAM);
    }

    // Body.
    rounded_rect(
        x - body / 2.0,
        cy - body / 2.0,
        body,
        body,
        26.0 * scale,
        INK,
    );
    rounded_rect(
        x - body / 2.0 + 5.0 * scale,
        cy - body / 2.0 + 5.0 * scale,
        body - 10.0 * scale,
        body - 10.0 * scale,
        22.0 * scale,
        YELLOW,
    );
    rounded_rect(
        x - body / 2.0 + 14.0 * scale,
        cy - body / 2.0 + 12.0 * scale,
        body - 28.0 * scale,
        6.0 * scale,
        3.0 * scale,
        alpha(CREAM, 0.6),
    );

    // Eyes.
    let blink = (t % 4.2) < 0.13;
    let eye_dx = 24.0 * scale;
    let eye_y = cy - 12.0 * scale;
    for side in [-1.0f32, 1.0] {
        let ex = x + side * eye_dx;
        if blink {
            draw_line(
                ex - 10.0 * scale,
                eye_y,
                ex + 10.0 * scale,
                eye_y,
                5.0 * scale,
                INK,
            );
        } else {
            draw_ellipse(ex, eye_y, 13.0 * scale, 16.0 * scale, 0.0, CREAM);
            draw_circle(ex, eye_y, 13.0 * scale, INK);
            draw_ellipse(ex, eye_y, 11.0 * scale, 14.0 * scale, 0.0, CREAM);
            let pupil = match mood {
                Mood::Happy | Mood::Excited => vec2(ex - 2.0 * scale, eye_y + 1.0 * scale),
                Mood::Sad => vec2(ex - 3.0 * scale, eye_y + 4.0 * scale),
                _ => vec2(ex - 2.0 * scale, eye_y + 2.0 * scale),
            };
            draw_circle(pupil.x, pupil.y, 6.0 * scale, INK);
            draw_circle(
                pupil.x - 2.0 * scale,
                pupil.y - 2.0 * scale,
                2.0 * scale,
                CREAM,
            );
        }
    }

    // Eyebrows.
    let brow_y = eye_y - 26.0 * scale;
    match mood {
        Mood::Worried => {
            draw_line(
                x - 40.0 * scale,
                brow_y - 4.0 * scale,
                x - 12.0 * scale,
                brow_y + 2.0 * scale,
                5.0 * scale,
                INK,
            );
            draw_line(
                x + 12.0 * scale,
                brow_y + 2.0 * scale,
                x + 40.0 * scale,
                brow_y - 4.0 * scale,
                5.0 * scale,
                INK,
            );
        }
        Mood::Sad => {
            draw_line(
                x - 40.0 * scale,
                brow_y + 2.0 * scale,
                x - 12.0 * scale,
                brow_y - 2.0 * scale,
                5.0 * scale,
                INK,
            );
            draw_line(
                x + 12.0 * scale,
                brow_y - 2.0 * scale,
                x + 40.0 * scale,
                brow_y + 2.0 * scale,
                5.0 * scale,
                INK,
            );
        }
        _ => {
            draw_line(
                x - 38.0 * scale,
                brow_y,
                x - 14.0 * scale,
                brow_y - 3.0 * scale,
                5.0 * scale,
                INK,
            );
            draw_line(
                x + 14.0 * scale,
                brow_y - 3.0 * scale,
                x + 38.0 * scale,
                brow_y,
                5.0 * scale,
                INK,
            );
        }
    }

    // Cheeks.
    for side in [-1.0f32, 1.0] {
        draw_circle(
            x + side * 42.0 * scale,
            cy + 16.0 * scale,
            9.0 * scale,
            alpha(PINK, 0.55),
        );
    }

    // Mouth.
    let mouth_y = cy + 30.0 * scale;
    match mood {
        Mood::Happy => {
            draw_ellipse(x, mouth_y, 24.0 * scale, 18.0 * scale, 0.0, INK);
            draw_ellipse(
                x,
                mouth_y + 5.0 * scale,
                15.0 * scale,
                9.0 * scale,
                0.0,
                PINK,
            );
        }
        Mood::Excited => {
            draw_ellipse(x, mouth_y, 28.0 * scale, 24.0 * scale, 0.0, INK);
            draw_ellipse(
                x,
                mouth_y + 7.0 * scale,
                16.0 * scale,
                10.0 * scale,
                0.0,
                PINK,
            );
            draw_rectangle(
                x - 14.0 * scale,
                mouth_y - 18.0 * scale,
                28.0 * scale,
                8.0 * scale,
                CREAM,
            );
        }
        Mood::Worried => {
            draw_ellipse(x, mouth_y, 12.0 * scale, 14.0 * scale, 0.0, INK);
        }
        Mood::Sad => {
            arc(
                x,
                mouth_y + 6.0 * scale,
                22.0 * scale,
                std::f32::consts::PI + 0.35,
                std::f32::consts::TAU - 0.35,
                6.0 * scale,
                INK,
            );
            draw_circle(x + 34.0 * scale, eye_y + 22.0 * scale, 4.0 * scale, CYAN);
        }
        Mood::Idle => {
            arc(
                x,
                mouth_y - 2.0 * scale,
                22.0 * scale,
                0.35,
                std::f32::consts::PI - 0.35,
                6.0 * scale,
                INK,
            );
        }
    }

    if mood == Mood::Worried {
        // Sweat drop.
        draw_circle(x + 52.0 * scale, cy - 34.0 * scale, 7.0 * scale, CYAN);
        draw_triangle(
            vec2(x + 52.0 * scale, cy - 50.0 * scale),
            vec2(x + 45.0 * scale, cy - 36.0 * scale),
            vec2(x + 59.0 * scale, cy - 36.0 * scale),
            CYAN,
        );
    }
}

// ---------------------------------------------------------------------------
// Board and HUD
// ---------------------------------------------------------------------------

fn draw_board(game: &Game, t: f32) {
    // Bezel and well.
    sticker(466.0, 96.0, 348.0, 668.0, PAPER);
    rounded_rect(472.0, 104.0, 336.0, 656.0, 12.0, INK);
    rounded_rect(478.0, 110.0, 324.0, 644.0, 8.0, WELL);

    // Faint grid.
    for i in 1..10 {
        let x = BOARD_X + i as f32 * CELL;
        draw_line(
            x,
            BOARD_Y,
            x,
            BOARD_Y + 20.0 * CELL,
            1.0,
            alpha(CREAM, 0.07),
        );
    }
    for j in 1..20 {
        let y = BOARD_Y + j as f32 * CELL;
        draw_line(
            BOARD_X,
            y,
            BOARD_X + 10.0 * CELL,
            y,
            1.0,
            alpha(CREAM, 0.07),
        );
    }

    // Locked cells.
    for (y, row) in game.board().iter().enumerate().take(20) {
        for (x, &id) in row.iter().enumerate() {
            if id != 0 {
                draw_block(
                    BOARD_X + x as f32 * CELL + 1.0,
                    BOARD_Y + y as f32 * CELL + 1.0,
                    CELL - 2.0,
                    id,
                    1.0,
                );
            }
        }
    }

    // Line-clear flash.
    if game.phase == Phase::Clearing {
        let k = (game.clear_timer / crate::game::CLEAR_ANIM).clamp(0.0, 1.0);
        for &row in &game.clearing {
            let y = BOARD_Y + row as f32 * CELL;
            draw_rectangle(
                BOARD_X,
                y,
                10.0 * CELL,
                CELL,
                alpha(CREAM, 0.35 + 0.45 * (k * 12.0).sin().abs()),
            );
            for i in 0..5 {
                let sx = BOARD_X + i as f32 * 64.0 + (t * 900.0) % 64.0;
                draw_line(sx, y + 6.0, sx + 30.0, y + 6.0, 3.0, alpha(CREAM, 0.8 * k));
            }
        }
    }

    // Ghost.
    if let Some(piece) = game.current() {
        if let Some(gy) = game.ghost_y() {
            if gy != piece.y {
                for &(cx, cy) in piece.cells().iter() {
                    let bx = piece.x + cx;
                    let by = gy + cy;
                    if by >= 0 {
                        draw_ghost_block(
                            BOARD_X + bx as f32 * CELL + 1.0,
                            BOARD_Y + by as f32 * CELL + 1.0,
                            CELL - 2.0,
                            piece.kind.id(),
                        );
                    }
                }
            }
        }
    }

    // Active piece with a cartoon wobble.
    if let Some(piece) = game.current() {
        let wobble = (t * 9.0).sin() * 0.8;
        for &(cx, cy) in piece.cells().iter() {
            let bx = piece.x + cx;
            let by = piece.y + cy;
            if by >= 0 {
                let x = BOARD_X + bx as f32 * CELL + 1.0 + wobble;
                let y = BOARD_Y + by as f32 * CELL + 1.0;
                rounded_rect(
                    x - 2.0,
                    y - 2.0,
                    CELL + 2.0,
                    CELL + 2.0,
                    8.0,
                    alpha(kind_color(piece.kind), 0.28),
                );
                draw_block(x, y, CELL - 2.0, piece.kind.id(), 1.0);
            }
        }
    }
}

fn draw_hold_panel(game: &Game, font: &Font) {
    sticker(48.0, 96.0, 360.0, 190.0, PAPER);
    tab_label(62.0, 110.0, "HOLD", YELLOW, font);
    if let Some(kind) = game.hold() {
        draw_preview(kind, 228.0, 200.0, 30.0);
    } else {
        text_center("EMPTY", 228.0, 200.0, 24.0, alpha(INK, 0.35), font, 0.0);
    }
}

fn draw_next_panel(game: &Game, font: &Font) {
    sticker(830.0, 96.0, 390.0, 300.0, PAPER);
    tab_label(844.0, 110.0, "NEXT", CYAN, font);
    let mut y = 168.0;
    for kind in game.queue().iter().take(3) {
        rounded_rect(852.0, y - 8.0, 346.0, 78.0, 12.0, alpha(INK, 0.08));
        draw_preview(*kind, 1025.0, y + 32.0, 24.0);
        y += 84.0;
    }
}

fn draw_stats_panel(game: &Game, high_score: u64, font: &Font) {
    sticker(830.0, 420.0, 390.0, 120.0, PAPER);
    tab_label(844.0, 432.0, "SCORE", PINK, font);
    text_center(
        &format!("{:07}", game.score),
        1025.0,
        510.0,
        42.0,
        INK,
        font,
        0.0,
    );

    sticker(830.0, 560.0, 185.0, 100.0, PAPER);
    tab_label(840.0, 570.0, "LINES", GREEN, font);
    text_center(
        &format!("{}", game.lines),
        922.0,
        632.0,
        34.0,
        INK,
        font,
        0.0,
    );

    sticker(1035.0, 560.0, 185.0, 100.0, PAPER);
    tab_label(1045.0, 570.0, "LEVEL", ORANGE, font);
    text_center(
        &format!("{}", game.level),
        1127.0,
        632.0,
        34.0,
        INK,
        font,
        0.0,
    );

    sticker(830.0, 680.0, 390.0, 90.0, PAPER);
    tab_label(844.0, 690.0, "BEST", PURPLE, font);
    text_center(
        &format!("{:07}", high_score.max(game.score)),
        1025.0,
        752.0,
        28.0,
        INK,
        font,
        0.0,
    );
}

fn draw_mascot_panel(assets: &Assets, mood: Mood, t: f32) {
    sticker(48.0, 320.0, 360.0, 450.0, PAPER);
    tab_label(62.0, 334.0, "BLOCKY", ORANGE, &assets.font);

    let mood_text = match mood {
        Mood::Idle => "STAY SHARP!",
        Mood::Happy => "NICE ONE!",
        Mood::Worried => "LOOK OUT!",
        Mood::Sad => "AWW...",
        Mood::Excited => "TETRIS!",
    };
    speech_bubble(228.0, 470.0, mood_text, &assets.font);
    draw_mascot(228.0, 590.0, 1.0, mood, t, false);

    // Decorative floor line.
    rounded_rect(90.0, 700.0, 276.0, 6.0, 3.0, alpha(INK, 0.15));
}

fn draw_header(assets: &Assets, music_on: bool, high_score: u64) {
    // Title.
    let letters = ["T", "E", "T", "R", "I", "S"];
    let colors = [YELLOW, CYAN, PINK, GREEN, PURPLE, ORANGE];
    let size = 54.0;
    let mut widths = Vec::new();
    for letter in letters {
        widths.push(measure_text(letter, Some(&assets.font), size as u16, 1.0).width);
    }
    let gap = 4.0;
    let total: f32 = widths.iter().sum::<f32>() + gap * (letters.len() - 1) as f32;
    let mut x = 640.0 - total / 2.0;
    for (i, letter) in letters.iter().enumerate() {
        let rot = if i % 2 == 0 { -0.04 } else { 0.05 };
        text_outlined(letter, x, 62.0, size, colors[i], &assets.font, 3.0, rot);
        x += widths[i] + gap;
    }
    text_center(
        "RETRO CARTOON EDITION",
        640.0,
        82.0,
        15.0,
        alpha(CREAM, 0.8),
        &assets.font,
        1.0,
    );

    text_outlined(
        if music_on { "MUSIC: ON" } else { "MUSIC: OFF" },
        48.0,
        60.0,
        18.0,
        alpha(CREAM, 0.85),
        &assets.font,
        1.0,
        0.0,
    );
    let best = format!("BEST {:07}", high_score);
    let dims = measure_text(&best, Some(&assets.font), 18, 1.0);
    text_outlined(
        &best,
        WINDOW_W - 48.0 - dims.width,
        60.0,
        18.0,
        alpha(CREAM, 0.85),
        &assets.font,
        1.0,
        0.0,
    );
}

// ---------------------------------------------------------------------------
// Screens
// ---------------------------------------------------------------------------

pub fn begin_shake(shake: Vec2) {
    let camera = Camera2D {
        target: vec2(WINDOW_W / 2.0 + shake.x, WINDOW_H / 2.0 + shake.y),
        zoom: vec2(2.0 / WINDOW_W, -2.0 / WINDOW_H),
        ..Default::default()
    };
    set_camera(&camera);
}

pub fn end_shake() {
    set_default_camera();
}

pub fn draw_scene(scene: &Scene) {
    let assets = scene.assets;
    draw_texture(&assets.background, 0.0, 0.0, WHITE);

    match scene.screen {
        Screen::Menu => draw_menu(scene),
        Screen::Playing => draw_game_screen(scene),
        Screen::Paused => {
            draw_game_screen(scene);
            draw_pause_overlay(scene);
        }
        Screen::GameOver => {
            draw_game_screen(scene);
            draw_game_over_overlay(scene);
        }
    }

    if scene.flash > 0.0 {
        draw_rectangle(
            0.0,
            0.0,
            WINDOW_W,
            WINDOW_H,
            Color::new(1.0, 1.0, 1.0, scene.flash * 0.5),
        );
    }
}

pub fn draw_post(scene: &Scene) {
    draw_texture(&scene.assets.overlay, 0.0, 0.0, WHITE);
}

fn draw_game_screen(scene: &Scene) {
    let assets = scene.assets;
    draw_header(assets, scene.music_on, scene.high_score);
    draw_hold_panel(scene.game, &assets.font);
    draw_mascot_panel(assets, scene.mood, scene.t);
    draw_board(scene.game, scene.t);
    draw_next_panel(scene.game, &assets.font);
    draw_stats_panel(scene.game, scene.high_score, &assets.font);
    scene.particles.draw();

    if let Some(toast) = scene.toast {
        if toast.active() {
            let k = (toast.timer / toast.total).clamp(0.0, 1.0);
            let pop = 1.0 + (1.0 - k) * 0.08;
            burst(
                640.0,
                220.0,
                108.0 * pop,
                toast.color,
                &toast.text,
                &assets.font,
                scene.t,
            );
        }
    }
}

fn draw_menu(scene: &Scene) {
    let assets = scene.assets;

    // Floating decorative tetrominoes.
    let floats: [(PieceKind, f32, f32, f32, f32); 6] = [
        (PieceKind::T, 120.0, 200.0, 44.0, 0.3),
        (PieceKind::S, 1160.0, 250.0, 46.0, -0.4),
        (PieceKind::I, 160.0, 600.0, 38.0, 0.6),
        (PieceKind::L, 1110.0, 640.0, 42.0, -0.25),
        (PieceKind::O, 330.0, 120.0, 30.0, 0.15),
        (PieceKind::Z, 980.0, 110.0, 34.0, 0.5),
    ];
    for (i, (kind, x, y, cell, rot)) in floats.iter().enumerate() {
        let angle = rot + (scene.t * 0.4 + i as f32).sin() * 0.12;
        draw_piece_rotated(*kind, *x, *y, *cell, angle, 0.35);
    }

    // Logo.
    let letters = ["T", "E", "T", "R", "I", "S"];
    let colors = [YELLOW, CYAN, PINK, GREEN, PURPLE, ORANGE];
    let size = 124.0;
    let mut widths = Vec::new();
    for letter in letters {
        widths.push(measure_text(letter, Some(&assets.font), size as u16, 1.0).width);
    }
    let gap = 6.0;
    let total: f32 = widths.iter().sum::<f32>() + gap * (letters.len() - 1) as f32;
    let mut x = 640.0 - total / 2.0;
    for (i, letter) in letters.iter().enumerate() {
        let rot = if i % 2 == 0 { -0.06 } else { 0.07 };
        // Chunky shadow.
        text_outlined(
            letter,
            x + 6.0,
            218.0 + 8.0,
            size,
            INK,
            &assets.font,
            5.0,
            rot,
        );
        text_outlined(letter, x, 218.0, size, colors[i], &assets.font, 5.0, rot);
        x += widths[i] + gap;
    }
    text_center(
        "RETRO CARTOON EDITION",
        640.0,
        262.0,
        24.0,
        CREAM,
        &assets.font,
        2.0,
    );

    // Menu card.
    sticker(250.0, 330.0, 780.0, 320.0, PAPER);
    let blink = ((scene.t * 2.4).sin() + 1.0) * 0.5;
    text_center(
        "PRESS ENTER TO START",
        640.0,
        390.0,
        44.0,
        blend(INK, PINK, blink * 0.5),
        &assets.font,
        3.0,
    );

    let level_text = format!("START LEVEL  < {} >", scene.start_level);
    text_center(&level_text, 640.0, 452.0, 30.0, INK, &assets.font, 0.0);
    text_center(
        "LEFT / RIGHT TO CHANGE LEVEL",
        640.0,
        480.0,
        16.0,
        alpha(INK, 0.55),
        &assets.font,
        0.0,
    );

    text_center(
        "ARROWS MOVE   DOWN SOFT DROP   SPACE HARD DROP",
        640.0,
        540.0,
        20.0,
        INK,
        &assets.font,
        0.0,
    );
    text_center(
        "UP / X ROTATE   Z ROTATE CCW   C HOLD   P PAUSE   M MUSIC",
        640.0,
        574.0,
        20.0,
        INK,
        &assets.font,
        0.0,
    );

    // Mascot and speech.
    speech_bubble(1120.0, 520.0, "LET'S PLAY!", &assets.font);
    draw_mascot(1120.0, 620.0, 1.1, Mood::Happy, scene.t, true);

    // Best score badge.
    sticker(48.0, 96.0, 300.0, 96.0, PAPER);
    tab_label(62.0, 106.0, "HIGH SCORE", PINK, &assets.font);
    text_center(
        &format!("{:07}", scene.high_score),
        198.0,
        158.0,
        34.0,
        INK,
        &assets.font,
        0.0,
    );

    // Music state.
    sticker(932.0, 96.0, 300.0, 96.0, PAPER);
    tab_label(946.0, 106.0, "MUSIC", CYAN, &assets.font);
    text_center(
        if scene.music_on { "ON" } else { "OFF" },
        1082.0,
        158.0,
        30.0,
        INK,
        &assets.font,
        0.0,
    );
}

fn draw_pause_overlay(scene: &Scene) {
    draw_rectangle(0.0, 0.0, WINDOW_W, WINDOW_H, alpha(INK, 0.66));
    burst(
        640.0,
        360.0,
        150.0,
        YELLOW,
        "PAUSED",
        &scene.assets.font,
        scene.t,
    );
    text_center(
        "PRESS P TO RESUME",
        640.0,
        540.0,
        28.0,
        CREAM,
        &scene.assets.font,
        2.0,
    );
    text_center(
        "R RESTART    M MUSIC",
        640.0,
        580.0,
        20.0,
        alpha(CREAM, 0.8),
        &scene.assets.font,
        1.0,
    );
}

fn draw_game_over_overlay(scene: &Scene) {
    draw_rectangle(0.0, 0.0, WINDOW_W, WINDOW_H, alpha(INK, 0.72));
    burst(
        640.0,
        250.0,
        170.0,
        RED,
        "GAME OVER",
        &scene.assets.font,
        scene.t,
    );

    sticker(420.0, 380.0, 440.0, 220.0, PAPER);
    text_center(
        "SCORE",
        640.0,
        420.0,
        24.0,
        alpha(INK, 0.6),
        &scene.assets.font,
        0.0,
    );
    text_center(
        &format!("{:07}", scene.game.score),
        640.0,
        480.0,
        50.0,
        INK,
        &scene.assets.font,
        0.0,
    );
    text_center(
        &format!("BEST {:07}", scene.high_score.max(scene.game.score)),
        640.0,
        530.0,
        24.0,
        INK,
        &scene.assets.font,
        0.0,
    );
    text_center(
        "PRESS ENTER TO PLAY AGAIN",
        640.0,
        660.0,
        30.0,
        CREAM,
        &scene.assets.font,
        2.0,
    );
    text_center(
        "ESC FOR MENU",
        640.0,
        700.0,
        20.0,
        alpha(CREAM, 0.8),
        &scene.assets.font,
        1.0,
    );
}
