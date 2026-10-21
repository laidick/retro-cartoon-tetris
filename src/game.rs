//! Pure Tetris rules: SRS rotation with wall kicks, 7-bag randomizer, hold,
//! ghost, lock delay, scoring, levels and game-over detection.
//!
//! This module deliberately has no rendering or audio dependencies so it can
//! be unit tested without opening a window.

use std::collections::VecDeque;

pub const BOARD_W: usize = 10;
pub const BOARD_H: usize = 20;

/// Duration of the line-clear animation before rows are removed.
pub const CLEAR_ANIM: f32 = 0.34;
/// Time a piece may rest on the stack before it locks.
pub const LOCK_DELAY: f32 = 0.5;
/// Maximum number of lock-delay resets per piece (guideline rule).
pub const MAX_LOCK_RESETS: u8 = 15;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PieceKind {
    I,
    O,
    T,
    S,
    Z,
    J,
    L,
}

pub const ALL_KINDS: [PieceKind; 7] = [
    PieceKind::I,
    PieceKind::O,
    PieceKind::T,
    PieceKind::S,
    PieceKind::Z,
    PieceKind::J,
    PieceKind::L,
];

impl PieceKind {
    /// 1-based id stored in the board; 0 means empty.
    pub fn id(self) -> u8 {
        match self {
            PieceKind::I => 1,
            PieceKind::O => 2,
            PieceKind::T => 3,
            PieceKind::S => 4,
            PieceKind::Z => 5,
            PieceKind::J => 6,
            PieceKind::L => 7,
        }
    }
}

/// SRS cell offsets for every piece and rotation, in a 4x4 bounding box.
/// Rotation 0 is the spawn state; y grows downwards.
static SHAPES: [[[(i32, i32); 4]; 4]; 7] = [
    // I
    [
        [(0, 1), (1, 1), (2, 1), (3, 1)],
        [(2, 0), (2, 1), (2, 2), (2, 3)],
        [(0, 2), (1, 2), (2, 2), (3, 2)],
        [(1, 0), (1, 1), (1, 2), (1, 3)],
    ],
    // O
    [
        [(1, 0), (2, 0), (1, 1), (2, 1)],
        [(1, 0), (2, 0), (1, 1), (2, 1)],
        [(1, 0), (2, 0), (1, 1), (2, 1)],
        [(1, 0), (2, 0), (1, 1), (2, 1)],
    ],
    // T
    [
        [(1, 0), (0, 1), (1, 1), (2, 1)],
        [(1, 0), (1, 1), (2, 1), (1, 2)],
        [(0, 1), (1, 1), (2, 1), (1, 2)],
        [(1, 0), (0, 1), (1, 1), (1, 2)],
    ],
    // S
    [
        [(1, 0), (2, 0), (0, 1), (1, 1)],
        [(1, 0), (1, 1), (2, 1), (2, 2)],
        [(1, 0), (2, 0), (0, 1), (1, 1)],
        [(0, 0), (0, 1), (1, 1), (1, 2)],
    ],
    // Z
    [
        [(0, 0), (1, 0), (1, 1), (2, 1)],
        [(2, 0), (1, 1), (2, 1), (1, 2)],
        [(0, 0), (1, 0), (1, 1), (2, 1)],
        [(1, 0), (0, 1), (1, 1), (0, 2)],
    ],
    // J
    [
        [(0, 0), (0, 1), (1, 1), (2, 1)],
        [(1, 0), (2, 0), (1, 1), (1, 2)],
        [(0, 1), (1, 1), (2, 1), (2, 2)],
        [(1, 0), (1, 1), (0, 2), (1, 2)],
    ],
    // L
    [
        [(2, 0), (0, 1), (1, 1), (2, 1)],
        [(1, 0), (1, 1), (1, 2), (2, 2)],
        [(0, 1), (1, 1), (2, 1), (0, 2)],
        [(0, 0), (1, 0), (1, 1), (1, 2)],
    ],
];

fn shape_index(kind: PieceKind) -> usize {
    match kind {
        PieceKind::I => 0,
        PieceKind::O => 1,
        PieceKind::T => 2,
        PieceKind::S => 3,
        PieceKind::Z => 4,
        PieceKind::J => 5,
        PieceKind::L => 6,
    }
}

pub fn shape(kind: PieceKind, rot: u8) -> &'static [(i32, i32); 4] {
    &SHAPES[shape_index(kind)][(rot % 4) as usize]
}

/// SRS wall-kick offsets in SRS coordinates (positive y is up). The caller
/// converts to screen coordinates by negating y.
fn kicks(kind: PieceKind, from: u8, to: u8) -> &'static [(i32, i32)] {
    const JLSTZ: [[(i32, i32); 5]; 8] = [
        // 0 -> R
        [(0, 0), (-1, 0), (-1, 1), (0, -2), (-1, -2)],
        // R -> 0
        [(0, 0), (1, 0), (1, -1), (0, 2), (1, 2)],
        // R -> 2
        [(0, 0), (1, 0), (1, -1), (0, 2), (1, 2)],
        // 2 -> R
        [(0, 0), (-1, 0), (-1, 1), (0, -2), (-1, -2)],
        // 2 -> L
        [(0, 0), (1, 0), (1, 1), (0, -2), (1, -2)],
        // L -> 2
        [(0, 0), (-1, 0), (-1, -1), (0, 2), (-1, 2)],
        // L -> 0
        [(0, 0), (-1, 0), (-1, -1), (0, 2), (-1, 2)],
        // 0 -> L
        [(0, 0), (1, 0), (1, 1), (0, -2), (1, -2)],
    ];
    const I: [[(i32, i32); 5]; 8] = [
        [(0, 0), (-2, 0), (1, 0), (-2, -1), (1, 2)],
        [(0, 0), (2, 0), (-1, 0), (2, 1), (-1, -2)],
        [(0, 0), (-1, 0), (2, 0), (-1, 2), (2, -1)],
        [(0, 0), (1, 0), (-2, 0), (1, -2), (-2, 1)],
        [(0, 0), (2, 0), (-1, 0), (2, 1), (-1, -2)],
        [(0, 0), (-2, 0), (1, 0), (-2, -1), (1, 2)],
        [(0, 0), (1, 0), (-2, 0), (1, -2), (-2, 1)],
        [(0, 0), (-1, 0), (2, 0), (-1, 2), (2, -1)],
    ];
    const O: [(i32, i32); 1] = [(0, 0)];

    if kind == PieceKind::O {
        return &O;
    }
    let idx = ((from as usize) * 4 + (to as usize)) % 8;
    // The tables above are indexed by transition: 0->1, 1->0, 1->2, 2->1,
    // 2->3, 3->2, 3->0, 0->3.
    let transition = match (from, to) {
        (0, 1) => 0,
        (1, 0) => 1,
        (1, 2) => 2,
        (2, 1) => 3,
        (2, 3) => 4,
        (3, 2) => 5,
        (3, 0) => 6,
        (0, 3) => 7,
        _ => {
            let _ = idx;
            0
        }
    };
    if kind == PieceKind::I {
        &I[transition]
    } else {
        &JLSTZ[transition]
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Playing,
    Clearing,
    GameOver,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    Move,
    Rotate,
    SoftDrop,
    HardDrop { cells: u32 },
    Lock,
    Hold,
    Lines { n: u32, level_up: bool },
    LevelUp(u32),
    GameOver,
}

#[derive(Clone, Copy, Debug)]
pub struct Piece {
    pub kind: PieceKind,
    pub rot: u8,
    pub x: i32,
    pub y: i32,
}

impl Piece {
    pub fn cells(&self) -> [(i32, i32); 4] {
        *shape(self.kind, self.rot)
    }
}

/// Small xorshift RNG. Deterministic for a given seed; no external crates.
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Rng {
        Rng(if seed == 0 {
            0x9E37_79B9_7F4A_7C15
        } else {
            seed
        })
    }

    pub fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    fn below(&mut self, n: u64) -> u64 {
        self.next_u64() % n
    }
}

pub struct Game {
    board: [[u8; BOARD_W]; BOARD_H],
    current: Option<Piece>,
    hold: Option<PieceKind>,
    can_hold: bool,
    queue: VecDeque<PieceKind>,
    rng: Rng,
    pub score: u64,
    pub lines: u32,
    pub level: u32,
    pub start_level: u32,
    pub combo: u32,
    pub phase: Phase,
    pub clearing: Vec<usize>,
    pub clear_timer: f32,
    pub last_clear: u32,
    pub best_clear: u32,
    pub pieces: u32,
    lock_timer: f32,
    lock_resets: u8,
    gravity_timer: f32,
    events: Vec<Event>,
}

impl Game {
    pub fn new(start_level: u32, seed: u64) -> Game {
        let start_level = start_level.clamp(1, 15);
        let mut game = Game {
            board: [[0; BOARD_W]; BOARD_H],
            current: None,
            hold: None,
            can_hold: false,
            queue: VecDeque::new(),
            rng: Rng::new(seed),
            score: 0,
            lines: 0,
            level: start_level,
            start_level,
            combo: 0,
            phase: Phase::Playing,
            clearing: Vec::new(),
            clear_timer: 0.0,
            last_clear: 0,
            best_clear: 0,
            pieces: 0,
            lock_timer: 0.0,
            lock_resets: 0,
            gravity_timer: 0.0,
            events: Vec::new(),
        };
        game.refill_queue();
        game.spawn_next();
        game
    }

    pub fn board(&self) -> &[[u8; BOARD_W]; BOARD_H] {
        &self.board
    }

    pub fn current(&self) -> Option<Piece> {
        self.current
    }

    pub fn hold(&self) -> Option<PieceKind> {
        self.hold
    }

    pub fn queue(&self) -> &VecDeque<PieceKind> {
        &self.queue
    }

    pub fn events(&mut self) -> Vec<Event> {
        std::mem::take(&mut self.events)
    }

    pub fn gravity_interval(&self) -> f32 {
        gravity_interval(self.level)
    }

    pub fn is_grounded(&self) -> bool {
        match self.current {
            Some(p) => self.collides(p.kind, p.rot, p.x, p.y + 1),
            None => false,
        }
    }

    pub fn ghost_y(&self) -> Option<i32> {
        let p = self.current?;
        let mut y = p.y;
        while !self.collides(p.kind, p.rot, p.x, y + 1) {
            y += 1;
        }
        Some(y)
    }

    /// Highest occupied row measured from the floor (0 = empty field).
    pub fn stack_height(&self) -> usize {
        for y in 0..BOARD_H {
            if self.board[y].iter().any(|&c| c != 0) {
                return BOARD_H - y;
            }
        }
        0
    }

    pub fn update(&mut self, dt: f32) {
        if self.phase == Phase::GameOver {
            return;
        }
        if self.phase == Phase::Clearing {
            self.clear_timer -= dt;
            if self.clear_timer <= 0.0 {
                self.finish_clear();
            }
            return;
        }
        if self.current.is_none() {
            return;
        }

        if self.is_grounded() {
            self.lock_timer += dt;
            if self.lock_timer >= LOCK_DELAY {
                self.lock_piece();
            }
            return;
        }

        self.gravity_timer += dt;
        let interval = self.gravity_interval();
        while self.gravity_timer >= interval {
            self.gravity_timer -= interval;
            if !self.shift(0, 1) {
                break;
            }
        }
    }

    pub fn move_piece(&mut self, dx: i32) -> bool {
        if self.phase != Phase::Playing {
            return false;
        }
        let moved = self.shift(dx, 0);
        if moved {
            self.events.push(Event::Move);
        }
        moved
    }

    pub fn soft_drop(&mut self) -> bool {
        if self.phase != Phase::Playing {
            return false;
        }
        if self.shift(0, 1) {
            self.score += 1;
            self.events.push(Event::SoftDrop);
            true
        } else {
            false
        }
    }

    pub fn hard_drop(&mut self) -> bool {
        if self.phase != Phase::Playing {
            return false;
        }
        let Some(p) = self.current else {
            return false;
        };
        let mut y = p.y;
        while !self.collides(p.kind, p.rot, p.x, y + 1) {
            y += 1;
        }
        let cells = (y - p.y).max(0) as u32;
        self.current = Some(Piece { y, ..p });
        self.score += cells as u64 * 2;
        self.events.push(Event::HardDrop { cells });
        self.lock_piece();
        true
    }

    pub fn rotate(&mut self, dir: i32) -> bool {
        if self.phase != Phase::Playing {
            return false;
        }
        let Some(p) = self.current else {
            return false;
        };
        let from = p.rot % 4;
        let to = ((from as i32 + dir).rem_euclid(4)) as u8;
        if to == from {
            return false;
        }
        for &(dx, dy) in kicks(p.kind, from, to) {
            let nx = p.x + dx;
            let ny = p.y - dy;
            if !self.collides(p.kind, to, nx, ny) {
                self.current = Some(Piece {
                    kind: p.kind,
                    rot: to,
                    x: nx,
                    y: ny,
                });
                self.reset_lock_delay();
                self.events.push(Event::Rotate);
                return true;
            }
        }
        false
    }

    pub fn hold_piece(&mut self) -> bool {
        if self.phase != Phase::Playing || !self.can_hold {
            return false;
        }
        let Some(p) = self.current else {
            return false;
        };
        let held = self.hold.replace(p.kind);
        self.current = None;
        self.can_hold = false;
        match held {
            Some(kind) => self.spawn_kind(kind, false),
            None => self.spawn_next(),
        }
        // A piece that arrived via hold cannot be held again until it locks.
        self.can_hold = false;
        self.events.push(Event::Hold);
        true
    }

    fn shift(&mut self, dx: i32, dy: i32) -> bool {
        let Some(p) = self.current else {
            return false;
        };
        if self.collides(p.kind, p.rot, p.x + dx, p.y + dy) {
            return false;
        }
        self.current = Some(Piece {
            x: p.x + dx,
            y: p.y + dy,
            ..p
        });
        self.reset_lock_delay();
        true
    }

    fn reset_lock_delay(&mut self) {
        if self.is_grounded() && self.lock_resets < MAX_LOCK_RESETS {
            self.lock_timer = 0.0;
            self.lock_resets += 1;
        }
    }

    fn collides(&self, kind: PieceKind, rot: u8, x: i32, y: i32) -> bool {
        for &(cx, cy) in shape(kind, rot).iter() {
            let bx = x + cx;
            let by = y + cy;
            if bx < 0 || bx >= BOARD_W as i32 || by >= BOARD_H as i32 {
                return true;
            }
            if by >= 0 && self.board[by as usize][bx as usize] != 0 {
                return true;
            }
        }
        false
    }

    fn refill_queue(&mut self) {
        while self.queue.len() < 5 {
            let mut bag = ALL_KINDS;
            for i in (1..bag.len()).rev() {
                let j = self.rng.below((i + 1) as u64) as usize;
                bag.swap(i, j);
            }
            self.queue.extend(bag);
        }
    }

    fn spawn_next(&mut self) {
        if self.queue.is_empty() {
            self.refill_queue();
        }
        let Some(kind) = self.queue.pop_front() else {
            return;
        };
        self.refill_queue();
        self.spawn_kind(kind, true);
    }

    fn spawn_kind(&mut self, kind: PieceKind, from_queue: bool) {
        let piece = Piece {
            kind,
            rot: 0,
            x: 3,
            y: -2,
        };
        self.lock_timer = 0.0;
        self.lock_resets = 0;
        self.gravity_timer = 0.0;
        if self.collides(piece.kind, piece.rot, piece.x, piece.y) {
            self.current = None;
            self.phase = Phase::GameOver;
            self.events.push(Event::GameOver);
            return;
        }
        self.current = Some(piece);
        self.can_hold = from_queue;
        self.pieces += 1;
    }

    fn lock_piece(&mut self) {
        let Some(p) = self.current.take() else {
            return;
        };
        let mut locked_out = false;
        for &(cx, cy) in shape(p.kind, p.rot).iter() {
            let x = p.x + cx;
            let y = p.y + cy;
            if y < 0 {
                locked_out = true;
                continue;
            }
            if x >= 0 && x < BOARD_W as i32 && y < BOARD_H as i32 {
                self.board[y as usize][x as usize] = p.kind.id();
            }
        }
        self.can_hold = true;
        self.lock_timer = 0.0;
        self.lock_resets = 0;
        self.gravity_timer = 0.0;
        self.events.push(Event::Lock);

        if locked_out {
            self.phase = Phase::GameOver;
            self.events.push(Event::GameOver);
            return;
        }

        let mut rows = Vec::new();
        for y in 0..BOARD_H {
            if self.board[y].iter().all(|&c| c != 0) {
                rows.push(y);
            }
        }

        if rows.is_empty() {
            self.combo = 0;
            self.spawn_next();
            return;
        }

        let n = rows.len() as u32;
        let base: u64 = match n {
            1 => 100,
            2 => 300,
            3 => 500,
            _ => 800,
        };
        self.score += base * self.level as u64;
        self.combo += 1;
        if self.combo > 1 {
            self.score += 50 * (self.combo as u64 - 1) * self.level as u64;
        }
        self.lines += n;
        self.last_clear = n;
        self.best_clear = self.best_clear.max(n);
        let new_level = (self.start_level + self.lines / 10).min(99);
        let level_up = new_level > self.level;
        self.level = new_level;
        self.clearing = rows;
        self.clear_timer = CLEAR_ANIM;
        self.phase = Phase::Clearing;
        self.events.push(Event::Lines { n, level_up });
        if level_up {
            self.events.push(Event::LevelUp(self.level));
        }
    }

    fn finish_clear(&mut self) {
        let mut rows = std::mem::take(&mut self.clearing);
        rows.sort_unstable();
        for &row in rows.iter().rev() {
            for y in (1..=row).rev() {
                self.board[y] = self.board[y - 1];
            }
            self.board[0] = [0; BOARD_W];
        }
        self.clear_timer = 0.0;
        self.phase = Phase::Playing;
        self.spawn_next();
    }

    /// Test helper: set a single board cell.
    #[cfg(test)]
    pub fn fill_cell(&mut self, x: usize, y: usize, kind: PieceKind) {
        if x < BOARD_W && y < BOARD_H {
            self.board[y][x] = kind.id();
        }
    }

    #[cfg(test)]
    fn force_piece(&mut self, kind: PieceKind, rot: u8, x: i32, y: i32) {
        self.current = Some(Piece { kind, rot, x, y });
        self.phase = Phase::Playing;
        self.lock_timer = 0.0;
        self.lock_resets = 0;
        self.gravity_timer = 0.0;
    }
}

/// Guideline gravity curve, clamped to a playable minimum.
pub fn gravity_interval(level: u32) -> f32 {
    let l = level.clamp(1, 99) as f32;
    let base = (0.8 - (l - 1.0) * 0.007).max(0.05);
    base.powf(l - 1.0).clamp(0.03, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn game() -> Game {
        Game::new(1, 0xDEAD_BEEF)
    }

    #[test]
    fn bag_contains_each_piece_once() {
        let mut game = game();
        let mut seen = vec![game.current().expect("first piece").kind];
        while seen.len() < 7 {
            seen.push(
                game.queue
                    .pop_front()
                    .expect("first bag holds seven pieces"),
            );
        }
        for kind in ALL_KINDS {
            assert_eq!(
                seen.iter().filter(|&&k| k == kind).count(),
                1,
                "bag should contain one {kind:?}"
            );
        }
    }

    #[test]
    fn spawns_above_the_visible_field() {
        let game = game();
        let piece = game.current().expect("piece spawned");
        assert!(piece.y <= 0, "spawn should start above the field");
    }

    #[test]
    fn gravity_moves_piece_down() {
        let mut game = game();
        let start = game.current().unwrap().y;
        game.update(1.2);
        let now = game.current().unwrap().y;
        assert!(now > start, "gravity should move the piece down");
    }

    #[test]
    fn hard_drop_locks_and_advances_queue() {
        let mut game = game();
        let first = game.current().unwrap().kind;
        game.hard_drop();
        assert!(game.current().is_some(), "next piece should spawn");
        assert_ne!(game.current().unwrap().kind, first, "queue advances");
        assert!(game.score > 0, "hard drop scores points");
    }

    #[test]
    fn wall_kick_allows_rotation_at_the_wall() {
        let mut game = game();
        game.force_piece(PieceKind::T, 0, 0, 10);
        assert!(game.rotate(-1), "T should kick off the left wall");
    }

    #[test]
    fn rotation_is_blocked_when_boxed_in() {
        let mut game = game();
        game.force_piece(PieceKind::T, 0, 3, 10);
        for x in 0..BOARD_W {
            for y in 8..13 {
                game.fill_cell(x, y, PieceKind::O);
            }
        }
        // The T's own cells were overwritten; clear them again.
        game.force_piece(PieceKind::T, 0, 3, 10);
        assert!(!game.rotate(1), "rotation should fail in a sealed box");
    }

    #[test]
    fn single_line_clear_scores_and_removes_the_row() {
        let mut game = game();
        for x in 0..BOARD_W - 2 {
            game.fill_cell(x, BOARD_H - 1, PieceKind::O);
        }
        game.force_piece(PieceKind::O, 0, 7, BOARD_H as i32 - 2);
        game.hard_drop();
        assert_eq!(game.phase, Phase::Clearing);
        assert_eq!(game.last_clear, 1);
        let expected = 100;
        assert_eq!(game.score, expected);
        game.update(CLEAR_ANIM + 0.1);
        assert_eq!(game.phase, Phase::Playing);
        assert_eq!(game.lines, 1);
        assert!(
            game.board().iter().all(|row| row.contains(&0)),
            "no full rows should remain"
        );
    }

    #[test]
    fn tetris_scores_800_times_level() {
        let mut game = game();
        for y in BOARD_H - 4..BOARD_H {
            for x in 0..BOARD_W {
                if x != 4 {
                    game.fill_cell(x, y, PieceKind::O);
                }
            }
        }
        // Vertical I in the only empty column completes all four rows.
        game.force_piece(PieceKind::I, 1, 2, BOARD_H as i32 - 4);
        game.hard_drop();
        assert_eq!(game.last_clear, 4);
        assert_eq!(game.score, 800);
    }

    #[test]
    fn hold_is_limited_to_once_per_piece() {
        let mut game = game();
        assert!(game.hold_piece());
        assert!(!game.hold_piece(), "second hold must be rejected");
        game.hard_drop();
        assert!(game.hold_piece(), "hold resets for the next piece");
    }

    #[test]
    fn topping_out_ends_the_game() {
        let mut game = game();
        for y in 0..BOARD_H {
            for x in 0..BOARD_W {
                game.fill_cell(x, y, PieceKind::O);
            }
        }
        game.hard_drop();
        assert_eq!(game.phase, Phase::GameOver);
    }

    #[test]
    fn lock_delay_locks_a_grounded_piece() {
        let mut game = game();
        game.force_piece(PieceKind::O, 0, 3, BOARD_H as i32 - 2);
        assert!(game.is_grounded());
        game.update(LOCK_DELAY + 0.01);
        assert_eq!(game.board()[BOARD_H - 1][4], PieceKind::O.id());
        assert!(game.current().is_some(), "next piece should spawn");
    }

    #[test]
    fn ten_lines_advance_the_level() {
        let mut game = game();
        for _ in 0..10 {
            for x in 0..BOARD_W - 1 {
                game.fill_cell(x, BOARD_H - 1, PieceKind::O);
            }
            game.force_piece(PieceKind::O, 0, 7, BOARD_H as i32 - 2);
            game.hard_drop();
            game.update(CLEAR_ANIM + 0.1);
        }
        assert_eq!(game.lines, 10);
        assert_eq!(game.level, 2);
        assert!(gravity_interval(2) < gravity_interval(1));
    }

    #[test]
    fn hold_swaps_the_current_piece() {
        let mut game = game();
        let first = game.current().unwrap().kind;
        assert!(game.hold_piece());
        assert_eq!(game.hold(), Some(first));
        game.hard_drop();
        let third = game.current().unwrap().kind;
        assert!(game.hold_piece());
        assert_eq!(
            game.hold(),
            Some(third),
            "hold now stores the swapped-out piece"
        );
        assert_eq!(game.current().unwrap().kind, first, "held piece returns");
    }

    #[test]
    fn every_rotation_has_four_distinct_cells() {
        for kind in ALL_KINDS {
            for rot in 0..4 {
                let cells = shape(kind, rot);
                let mut unique = Vec::new();
                for &(x, y) in cells.iter() {
                    assert!((0..4).contains(&x) && (0..4).contains(&y));
                    assert!(
                        !unique.contains(&(x, y)),
                        "{kind:?} rot {rot} has duplicate cells"
                    );
                    unique.push((x, y));
                }
                assert_eq!(unique.len(), 4);
            }
        }
    }

    #[test]
    fn random_play_never_panics_and_keeps_the_board_valid() {
        let mut game = game();
        let mut rng = Rng::new(0xFEED_FACE);
        for _ in 0..4000 {
            match rng.next_u64() % 8 {
                0 => {
                    game.move_piece(-1);
                }
                1 => {
                    game.move_piece(1);
                }
                2 => {
                    game.rotate(1);
                }
                3 => {
                    game.rotate(-1);
                }
                4 => {
                    game.soft_drop();
                }
                5 => {
                    game.hard_drop();
                }
                6 => {
                    game.hold_piece();
                }
                _ => {}
            }
            game.update(0.016);
            let _ = game.events();
            for row in game.board().iter() {
                for &cell in row.iter() {
                    assert!(cell <= 7);
                }
            }
            if game.phase == Phase::GameOver {
                break;
            }
        }
        assert!(game.score > 0);
    }

    #[test]
    fn ghost_is_at_or_below_the_piece() {
        let game = game();
        let piece = game.current().unwrap();
        let ghost = game.ghost_y().unwrap();
        assert!(ghost >= piece.y);
    }
}
