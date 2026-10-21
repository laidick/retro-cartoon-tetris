//! Retro cartoon Tetris: game shell, input, audio triggers and screenshot mode.

use macroquad::prelude::*;

mod audio;
mod game;
mod render;

use audio::AudioBank;
use game::{Event, Game, Phase, Rng};
use render::{Assets, Mood, Particles, Scene, Screen, Toast, BOARD_Y, CELL, WINDOW_H, WINDOW_W};

fn window_conf() -> Conf {
    Conf {
        window_title: "TETRIS - Retro Cartoon Edition".to_owned(),
        window_width: WINDOW_W as i32,
        window_height: WINDOW_H as i32,
        high_dpi: false,
        sample_count: 4,
        window_resizable: false,
        ..Default::default()
    }
}

struct Args {
    shot: Option<String>,
    screen: String,
    mute: bool,
    audio_check: bool,
    dump_audio: Option<String>,
}

fn parse_args() -> Args {
    let args: Vec<String> = std::env::args().collect();
    let mut shot = None;
    let mut screen = String::from("play");
    let mut mute = false;
    let mut audio_check = false;
    let mut dump_audio = None;
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--shot" => {
                i += 1;
                shot = args.get(i).cloned();
            }
            "--screen" => {
                i += 1;
                screen = args.get(i).cloned().unwrap_or_else(|| "play".to_string());
            }
            "--mute" => mute = true,
            "--audio-check" => audio_check = true,
            "--dump-audio" => {
                i += 1;
                dump_audio = args.get(i).cloned();
            }
            _ => {}
        }
        i += 1;
    }
    Args {
        shot,
        screen,
        mute,
        audio_check,
        dump_audio,
    }
}

struct HorizontalRepeat {
    dir: i32,
    timer: f32,
    repeating: bool,
}

impl HorizontalRepeat {
    fn new() -> Self {
        HorizontalRepeat {
            dir: 0,
            timer: 0.0,
            repeating: false,
        }
    }

    fn reset(&mut self) {
        self.dir = 0;
        self.timer = 0.0;
        self.repeating = false;
    }
}

struct App {
    assets: Assets,
    audio: AudioBank,
    screen: Screen,
    game: Game,
    start_level: u32,
    high_score: u64,
    particles: Particles,
    toast: Option<Toast>,
    mood: Mood,
    mood_timer: f32,
    shake: f32,
    shake_offset: Vec2,
    flash: f32,
    t: f32,
    das: HorizontalRepeat,
    soft_timer: f32,
    running: bool,
    rng: Rng,
    mute: bool,
}

impl App {
    fn new(assets: Assets, audio: AudioBank, high_score: u64, mute: bool) -> App {
        let seed = seed_now();
        App {
            assets,
            audio,
            screen: Screen::Menu,
            game: Game::new(1, seed),
            start_level: 1,
            high_score,
            particles: Particles::new(),
            toast: None,
            mood: Mood::Idle,
            mood_timer: 0.0,
            shake: 0.0,
            shake_offset: Vec2::ZERO,
            flash: 0.0,
            t: 0.0,
            das: HorizontalRepeat::new(),
            soft_timer: 0.0,
            running: true,
            rng: Rng::new(seed ^ 0x9E37_79B9),
            mute,
        }
    }
}

fn seed_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0x1234_5678_9ABC_DEF0)
}

fn high_score_path() -> std::path::PathBuf {
    std::env::current_dir()
        .unwrap_or_else(|_| std::path::PathBuf::from("."))
        .join("highscore.txt")
}

fn load_high_score() -> u64 {
    std::fs::read_to_string(high_score_path())
        .ok()
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(0)
}

fn save_high_score(score: u64) {
    let _ = std::fs::write(high_score_path(), score.to_string());
}

#[macroquad::main(window_conf)]
async fn main() {
    let args = parse_args();
    let assets = Assets::load();
    let high_score = load_high_score();
    let audio = if args.shot.is_some() || args.mute {
        AudioBank::silent()
    } else {
        AudioBank::load().await
    };
    let mut app = App::new(assets, audio, high_score, args.mute);
    if app.mute {
        app.audio.set_muted(true);
    }

    if args.audio_check {
        println!("audio: {}", app.audio.report());
        return;
    }

    if let Some(dir) = &args.dump_audio {
        match AudioBank::dump_wavs(std::path::Path::new(dir)) {
            Ok(()) => println!("audio clips written to {dir}"),
            Err(e) => eprintln!("could not write audio clips: {e}"),
        }
        return;
    }

    if let Some(path) = args.shot.clone() {
        run_shot(&mut app, &path, &args.screen).await;
        return;
    }

    app.audio.start_music();
    while app.running {
        let dt = get_frame_time().min(0.05);
        tick(&mut app, dt);
        draw_frame(&app);
        next_frame().await;
    }
}

fn tick(app: &mut App, dt: f32) {
    app.t += dt;
    handle_input(app, dt);
    if app.screen == Screen::Playing {
        app.game.update(dt);
    }
    let events = app.game.events();
    process_events(app, events);

    if let Some(toast) = app.toast.as_mut() {
        toast.update(dt);
        if !toast.active() {
            app.toast = None;
        }
    }
    app.particles.update(dt);
    app.shake = (app.shake - dt * 30.0).max(0.0);
    app.flash = (app.flash - dt * 2.4).max(0.0);
    app.shake_offset = if app.shake > 0.2 {
        vec2(
            signed(&mut app.rng) * app.shake,
            signed(&mut app.rng) * app.shake,
        )
    } else {
        Vec2::ZERO
    };

    if app.screen == Screen::Playing {
        if app.mood_timer > 0.0 {
            app.mood_timer -= dt;
        } else {
            app.mood = if app.game.stack_height() >= 13 {
                Mood::Worried
            } else {
                Mood::Idle
            };
        }
    }
}

fn signed(rng: &mut Rng) -> f32 {
    (rng.next_u64() % 2000) as f32 / 1000.0 - 1.0
}

fn toggle_music(app: &mut App) {
    if app.screen == Screen::Paused {
        app.audio.music_on = !app.audio.music_on;
        if !app.audio.music_on {
            app.audio.stop_music();
        }
    } else {
        app.audio.toggle_music();
    }
    app.audio.play("ui");
}

fn start_game(app: &mut App) {
    let seed = seed_now() ^ ((app.start_level as u64) << 32);
    app.game = Game::new(app.start_level, seed);
    app.screen = Screen::Playing;
    app.das.reset();
    app.soft_timer = 0.0;
    app.toast = None;
    app.particles.clear();
    app.mood = Mood::Idle;
    app.mood_timer = 0.0;
    app.shake = 0.0;
    app.shake_offset = Vec2::ZERO;
    app.flash = 0.0;
    app.audio.start_music();
    app.audio.play("ui");
}

fn goto_menu(app: &mut App) {
    app.screen = Screen::Menu;
    app.das.reset();
    app.soft_timer = 0.0;
    app.toast = None;
    app.mood = Mood::Idle;
    app.mood_timer = 0.0;
    app.audio.start_music();
    app.audio.play("ui");
}

fn pause_game(app: &mut App) {
    app.screen = Screen::Paused;
    app.das.reset();
    app.soft_timer = 0.0;
    app.audio.stop_music();
    app.audio.play("pause");
}

fn resume_game(app: &mut App) {
    app.screen = Screen::Playing;
    app.das.reset();
    app.soft_timer = 0.0;
    app.audio.start_music();
    app.audio.play("ui");
}

fn handle_input(app: &mut App, dt: f32) {
    match app.screen {
        Screen::Menu => {
            if is_key_pressed(KeyCode::Enter) || is_key_pressed(KeyCode::Space) {
                start_game(app);
                return;
            }
            if is_key_pressed(KeyCode::Left) || is_key_pressed(KeyCode::Up) {
                app.start_level = (app.start_level - 1).clamp(1, 15);
                app.audio.play("ui");
            }
            if is_key_pressed(KeyCode::Right) || is_key_pressed(KeyCode::Down) {
                app.start_level = (app.start_level + 1).clamp(1, 15);
                app.audio.play("ui");
            }
            if is_key_pressed(KeyCode::M) {
                toggle_music(app);
            }
            if is_key_pressed(KeyCode::Escape) {
                app.running = false;
            }
        }
        Screen::Playing => {
            if is_key_pressed(KeyCode::Escape) || is_key_pressed(KeyCode::P) {
                pause_game(app);
                return;
            }
            if is_key_pressed(KeyCode::M) {
                toggle_music(app);
            }
            if is_key_pressed(KeyCode::R) {
                start_game(app);
                return;
            }

            // Horizontal auto-repeat (DAS/ARR).
            if is_key_pressed(KeyCode::Left) {
                app.das.dir = -1;
                app.das.timer = 0.0;
                app.das.repeating = false;
                app.game.move_piece(-1);
            } else if is_key_pressed(KeyCode::Right) {
                app.das.dir = 1;
                app.das.timer = 0.0;
                app.das.repeating = false;
                app.game.move_piece(1);
            }
            if app.das.dir != 0 {
                let held = if app.das.dir < 0 {
                    is_key_down(KeyCode::Left)
                } else {
                    is_key_down(KeyCode::Right)
                };
                if !held {
                    app.das.reset();
                } else {
                    app.das.timer += dt;
                    if !app.das.repeating && app.das.timer >= 0.17 {
                        app.das.repeating = true;
                        app.das.timer = 0.0;
                        app.game.move_piece(app.das.dir);
                    } else if app.das.repeating && app.das.timer >= 0.04 {
                        app.das.timer = 0.0;
                        app.game.move_piece(app.das.dir);
                    }
                }
            }

            if is_key_pressed(KeyCode::Down) {
                app.game.soft_drop();
                app.soft_timer = 0.0;
            } else if is_key_down(KeyCode::Down) {
                app.soft_timer += dt;
                if app.soft_timer >= 0.045 {
                    app.soft_timer = 0.0;
                    app.game.soft_drop();
                }
            }

            if is_key_pressed(KeyCode::Up) || is_key_pressed(KeyCode::X) {
                app.game.rotate(1);
            }
            if is_key_pressed(KeyCode::Z) || is_key_pressed(KeyCode::LeftControl) {
                app.game.rotate(-1);
            }
            if is_key_pressed(KeyCode::C) || is_key_pressed(KeyCode::LeftShift) {
                app.game.hold_piece();
            }
            if is_key_pressed(KeyCode::Space) {
                hard_drop_with_dust(app);
            }
        }
        Screen::Paused => {
            if is_key_pressed(KeyCode::P) || is_key_pressed(KeyCode::Escape) {
                resume_game(app);
            }
            if is_key_pressed(KeyCode::R) {
                start_game(app);
            }
            if is_key_pressed(KeyCode::M) {
                toggle_music(app);
            }
        }
        Screen::GameOver => {
            if is_key_pressed(KeyCode::Enter)
                || is_key_pressed(KeyCode::R)
                || is_key_pressed(KeyCode::Space)
            {
                start_game(app);
            }
            if is_key_pressed(KeyCode::Escape) {
                goto_menu(app);
            }
            if is_key_pressed(KeyCode::M) {
                toggle_music(app);
            }
        }
    }
}

fn hard_drop_with_dust(app: &mut App) {
    if let Some(piece) = app.game.current() {
        if let Some(ghost) = app.game.ghost_y() {
            for &(cx, cy) in piece.cells().iter() {
                let by = ghost + cy;
                if by >= 0 {
                    let x = render::BOARD_X + (piece.x + cx) as f32 * CELL + CELL / 2.0;
                    let y = BOARD_Y + by as f32 * CELL + CELL;
                    app.particles.dust(x, y, 3, render::dust_color());
                }
            }
        }
    }
    app.game.hard_drop();
}

fn process_events(app: &mut App, events: Vec<Event>) {
    for event in events {
        match event {
            Event::Move => app.audio.play("move"),
            Event::Rotate => app.audio.play("rotate"),
            Event::SoftDrop => {}
            Event::HardDrop { cells } => {
                app.audio.play("drop");
                app.shake = (app.shake + 3.0 + cells as f32 * 0.25).min(14.0);
            }
            Event::Lock => app.audio.play("lock"),
            Event::Hold => app.audio.play("hold"),
            Event::Lines { n, .. } => {
                let palette = render::party_colors();
                let (text, color) = match n {
                    4 => ("TETRIS!", palette[2]),
                    3 => ("TRIPLE!", palette[0]),
                    2 => ("DOUBLE!", palette[1]),
                    _ => ("CLEAR!", palette[3]),
                };
                app.audio.play(if n == 4 { "tetris" } else { "line" });
                app.toast = Some(Toast::new(text, color));
                app.shake = (app.shake + 6.0 + n as f32 * 2.0).min(20.0);
                app.flash = 0.5;
                app.mood = if n >= 3 { Mood::Excited } else { Mood::Happy };
                app.mood_timer = 2.6;
                for row in app.game.clearing.clone() {
                    let y = BOARD_Y + row as f32 * CELL + CELL / 2.0;
                    app.particles.confetti(640.0, y, 20, &palette);
                    app.particles.sparkle(640.0, y, 14);
                }
            }
            Event::LevelUp(level) => {
                app.audio.play("level");
                let palette = render::party_colors();
                app.toast = Some(Toast::new(&format!("LEVEL {}!", level), palette[0]));
                app.flash = 0.55;
                app.mood = Mood::Excited;
                app.mood_timer = 2.6;
                app.particles.confetti(640.0, 200.0, 60, &palette);
            }
            Event::GameOver => {
                app.screen = Screen::GameOver;
                app.audio.stop_music();
                app.audio.play("gameover");
                app.mood = Mood::Sad;
                app.mood_timer = 999.0;
                app.toast = None;
                if app.game.score > app.high_score {
                    app.high_score = app.game.score;
                    save_high_score(app.high_score);
                }
            }
        }
    }
}

fn draw_frame(app: &App) {
    let scene = Scene {
        assets: &app.assets,
        screen: app.screen,
        game: &app.game,
        high_score: app.high_score,
        start_level: app.start_level,
        music_on: app.audio.music_on,
        toast: app.toast.as_ref(),
        mood: app.mood,
        particles: &app.particles,
        shake: app.shake_offset,
        flash: app.flash,
        t: app.t,
    };
    render::begin_shake(scene.shake);
    render::draw_scene(&scene);
    render::end_shake();
    render::draw_post(&scene);
}

// ---------------------------------------------------------------------------
// Screenshot / demo mode
// ---------------------------------------------------------------------------

fn steer_to(game: &mut Game, target: i32) {
    for _ in 0..12 {
        let Some(piece) = game.current() else { break };
        if piece.x < target {
            if !game.move_piece(1) {
                break;
            }
        } else if piece.x > target {
            if !game.move_piece(-1) {
                break;
            }
        } else {
            break;
        }
    }
}

fn demo_game(seed: u64, drops: u32) -> Game {
    let mut game = Game::new(4, seed);
    let mut rng = Rng::new(seed ^ 0xA5A5_1234);
    for _ in 0..drops {
        if game.phase == Phase::GameOver {
            break;
        }
        if game.phase == Phase::Clearing {
            game.update(1.0);
            continue;
        }
        let target = (rng.next_u64() % 10) as i32;
        steer_to(&mut game, target);
        game.hard_drop();
    }
    if game.phase == Phase::Clearing {
        game.update(1.0);
    }
    let _ = game.events();
    game
}

async fn run_shot(app: &mut App, path: &str, screen: &str) {
    app.t = 1.1;
    match screen {
        "menu" => {
            app.screen = Screen::Menu;
        }
        "pause" => {
            app.game = demo_game(0xC0FFEE, 16);
            app.screen = Screen::Paused;
        }
        "gameover" => {
            app.game = demo_game(0xBEEF, 140);
            app.high_score = app.high_score.max(app.game.score);
            app.screen = Screen::GameOver;
            app.mood = Mood::Sad;
        }
        _ => {
            app.game = demo_game(0xC0FFEE, 16);
            app.game.score = 4820;
            app.game.lines = 23;
            app.game.level = 4;
            app.screen = Screen::Playing;
            app.toast = Some(Toast::new("TETRIS!", render::party_colors()[2]));
            app.mood = Mood::Excited;
            let palette = render::party_colors();
            app.particles.confetti(640.0, 260.0, 50, &palette);
            app.flash = 0.25;
        }
    }

    next_frame().await;
    draw_frame(app);
    let mut image = get_screen_data();
    flip_vertical(&mut image);
    image.export_png(path);
}

/// `get_screen_data` reads the framebuffer bottom-up, so exported shots need a
/// vertical flip to match what is on screen.
fn flip_vertical(image: &mut Image) {
    let row = image.width as usize * 4;
    let height = image.height as usize;
    for y in 0..height / 2 {
        let top = y * row;
        let bottom = (height - 1 - y) * row;
        for i in 0..row {
            image.bytes.swap(top + i, bottom + i);
        }
    }
}
