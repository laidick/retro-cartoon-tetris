//! Chiptune audio engine.
//!
//! Everything is synthesized at startup into 16-bit PCM WAV buffers and handed
//! to macroquad's audio mixer, so the game ships without external audio files.
//!
//! The music is the public-domain folk tune Korobeiniki, arranged the way the
//! original NES Tetris does it: two pulse voices and a triangle bass at
//! roughly 150 BPM. The sound effects follow the NES Tetris palette (short
//! pulse blips, noise thuds, rising clear sweeps and a tetris fanfare).

use macroquad::audio::{load_sound_from_bytes, play_sound, stop_sound, PlaySoundParams, Sound};

pub const SAMPLE_RATE: u32 = 44100;
const BPM: f32 = 150.0;

pub struct AudioBank {
    music: Option<Sound>,
    sfx: Vec<(&'static str, Sound)>,
    pub music_on: bool,
    pub music_playing: bool,
    pub muted: bool,
}

impl AudioBank {
    /// Load every generated clip. Missing audio hardware degrades to silence.
    pub async fn load() -> AudioBank {
        let music = load_sound_from_bytes(&wav_bytes(&build_music())).await.ok();
        let mut sfx = Vec::new();
        for (name, samples) in sfx_definitions() {
            if let Ok(sound) = load_sound_from_bytes(&wav_bytes(&samples)).await {
                sfx.push((name, sound));
            }
        }
        AudioBank {
            music,
            sfx,
            music_on: true,
            music_playing: false,
            muted: false,
        }
    }

    /// Write every generated clip to disk for inspection or external playback.
    pub fn dump_wavs(dir: &std::path::Path) -> std::io::Result<()> {
        std::fs::create_dir_all(dir)?;
        std::fs::write(dir.join("music.wav"), wav_bytes(&build_music()))?;
        for (name, samples) in sfx_definitions() {
            std::fs::write(dir.join(format!("{name}.wav")), wav_bytes(&samples))?;
        }
        Ok(())
    }

    pub fn report(&self) -> String {
        format!(
            "music={} sfx_clips={} music_on={} muted={}",
            self.music.is_some(),
            self.sfx.len(),
            self.music_on,
            self.muted
        )
    }

    /// Used by screenshot mode, where opening an audio device is unnecessary.
    pub fn silent() -> AudioBank {
        AudioBank {
            music: None,
            sfx: Vec::new(),
            music_on: false,
            music_playing: false,
            muted: true,
        }
    }

    pub fn play(&self, name: &str) {
        if self.muted {
            return;
        }
        if let Some((_, sound)) = self.sfx.iter().find(|(n, _)| *n == name) {
            play_sound(
                sound,
                PlaySoundParams {
                    looped: false,
                    volume: 0.7,
                },
            );
        }
    }

    pub fn start_music(&mut self) {
        if self.muted || !self.music_on || self.music_playing {
            return;
        }
        if let Some(music) = &self.music {
            play_sound(
                music,
                PlaySoundParams {
                    looped: true,
                    volume: 0.32,
                },
            );
            self.music_playing = true;
        }
    }

    pub fn stop_music(&mut self) {
        if let Some(music) = &self.music {
            stop_sound(music);
        }
        self.music_playing = false;
    }

    pub fn toggle_music(&mut self) -> bool {
        self.music_on = !self.music_on;
        if self.music_on {
            self.start_music();
        } else {
            self.stop_music();
        }
        self.music_on
    }

    /// Muting is only used by screenshot mode and the `--mute` flag.
    pub fn set_muted(&mut self, muted: bool) {
        self.muted = muted;
        if muted {
            self.stop_music();
        }
    }
}

// ---------------------------------------------------------------------------
// WAV encoding
// ---------------------------------------------------------------------------

fn wav_bytes(samples: &[f32]) -> Vec<u8> {
    let data_len = (samples.len() * 2) as u32;
    let mut out = Vec::with_capacity(44 + data_len as usize);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data_len).to_le_bytes());
    out.extend_from_slice(b"WAVE");
    out.extend_from_slice(b"fmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes()); // PCM
    out.extend_from_slice(&1u16.to_le_bytes()); // mono
    out.extend_from_slice(&SAMPLE_RATE.to_le_bytes());
    out.extend_from_slice(&(SAMPLE_RATE * 2).to_le_bytes()); // byte rate
    out.extend_from_slice(&2u16.to_le_bytes()); // block align
    out.extend_from_slice(&16u16.to_le_bytes()); // bits per sample
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_len.to_le_bytes());
    for &s in samples {
        let v = (s.clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
        out.extend_from_slice(&v.to_le_bytes());
    }
    out
}

// ---------------------------------------------------------------------------
// Music
// ---------------------------------------------------------------------------

#[derive(Clone, Copy)]
enum Voice {
    Lead,
    Bass,
}

struct Note {
    start: f32,
    dur: f32,
    freq: Option<f32>,
    voice: Voice,
}

fn note_freq(name: &str) -> Option<f32> {
    let bytes = name.as_bytes();
    if bytes.is_empty() {
        return None;
    }
    let semitone = match bytes[0] as char {
        'C' => 0,
        'D' => 2,
        'E' => 4,
        'F' => 5,
        'G' => 7,
        'A' => 9,
        'B' => 11,
        _ => return None,
    };
    let mut idx = 1;
    let mut sharp = 0;
    if idx < bytes.len() && bytes[idx] == b'#' {
        sharp = 1;
        idx += 1;
    }
    let octave: i32 = std::str::from_utf8(&bytes[idx..]).ok()?.parse().ok()?;
    let midi = (octave + 1) * 12 + semitone + sharp;
    Some(440.0 * 2f32.powf((midi as f32 - 69.0) / 12.0))
}

fn parse_sequence(seq: &str, sixteenth: f32, voice: Voice) -> Vec<Note> {
    let mut notes = Vec::new();
    let mut cursor = 0.0f32;
    for token in seq.split_whitespace() {
        if token == "|" {
            continue;
        }
        let Some((name, len)) = token.split_once(':') else {
            continue;
        };
        let Ok(len) = len.parse::<u32>() else {
            continue;
        };
        let dur = len as f32 * sixteenth;
        if name != "R" {
            if let Some(freq) = note_freq(name) {
                notes.push(Note {
                    start: cursor,
                    dur,
                    freq: Some(freq),
                    voice,
                });
            }
        }
        cursor += dur;
    }
    notes
}

/// Korobeiniki (Tetris Type A) melody. Bars are separated by `|` and each
/// bar is 16 sixteenth-notes long at 150 BPM.
const LEAD: &str = "\
E5:2 B4:2 C5:2 D5:2 C5:2 B4:2 A4:2 A4:2 | \
C5:2 E5:2 D5:2 C5:2 B4:2 C5:2 D5:2 E5:2 | \
C5:2 A4:2 A4:4 R:8 | \
D5:2 F5:2 A5:2 G5:2 F5:2 E5:2 C5:2 E5:2 | \
D5:2 C5:2 B4:2 B4:2 C5:2 D5:2 E5:2 C5:2 | \
A4:2 A4:2 R:12 | \
E5:2 C5:2 D5:2 B4:2 C5:2 A4:2 G#4:2 B4:2 | \
E5:2 C5:2 D5:2 B4:2 C5:2 E5:2 A5:2 G#5:2 | \
E5:2 B4:2 C5:2 D5:2 C5:2 B4:2 A4:2 A4:2 | \
C5:2 E5:2 D5:2 C5:2 B4:2 C5:2 D5:2 E5:2 | \
C5:2 A4:2 A4:4 R:8 | \
D5:2 F5:2 A5:2 G5:2 F5:2 E5:2 C5:2 E5:2 | \
D5:2 C5:2 B4:2 B4:2 C5:2 D5:2 E5:2 C5:2 | \
A4:2 A4:2 R:12 | \
E5:2 C5:2 D5:2 B4:2 C5:2 A4:2 G#4:2 B4:2 | \
E5:2 C5:2 D5:2 B4:2 C5:2 E5:2 A5:2 G#5:2";

const BASS: &str = "\
A2:2 A3:2 A2:2 A3:2 A2:2 A3:2 A2:2 A3:2 | \
A2:2 A3:2 A2:2 A3:2 A2:2 A3:2 A2:2 A3:2 | \
A2:2 A3:2 A2:2 A3:2 A2:2 A3:2 A2:2 A3:2 | \
D2:2 D3:2 D2:2 D3:2 D2:2 D3:2 D2:2 D3:2 | \
A2:2 A3:2 A2:2 A3:2 A2:2 A3:2 A2:2 A3:2 | \
E2:2 E3:2 E2:2 E3:2 E2:2 E3:2 E2:2 E3:2 | \
A2:2 A3:2 A2:2 A3:2 A2:2 A3:2 A2:2 A3:2 | \
E2:2 E3:2 E2:2 E3:2 E2:2 E3:2 E2:2 E3:2 | \
A2:2 A3:2 A2:2 A3:2 A2:2 A3:2 A2:2 A3:2 | \
A2:2 A3:2 A2:2 A3:2 A2:2 A3:2 A2:2 A3:2 | \
A2:2 A3:2 A2:2 A3:2 A2:2 A3:2 A2:2 A3:2 | \
D2:2 D3:2 D2:2 D3:2 D2:2 D3:2 D2:2 D3:2 | \
A2:2 A3:2 A2:2 A3:2 A2:2 A3:2 A2:2 A3:2 | \
E2:2 E3:2 E2:2 E3:2 E2:2 E3:2 E2:2 E3:2 | \
A2:2 A3:2 A2:2 A3:2 A2:2 A3:2 A2:2 A3:2 | \
E2:2 E3:2 E2:2 E3:2 E2:2 E3:2 E2:2 E3:2";

fn build_music() -> Vec<f32> {
    let sixteenth = 60.0 / BPM / 4.0;
    let lead = parse_sequence(LEAD, sixteenth, Voice::Lead);
    let bass = parse_sequence(BASS, sixteenth, Voice::Bass);
    let total = 16.0 * 16.0 * sixteenth;
    let mut buffer = vec![0.0f32; (total * SAMPLE_RATE as f32) as usize + 1];

    for note in lead.iter().chain(bass.iter()) {
        let Some(freq) = note.freq else { continue };
        let start = (note.start * SAMPLE_RATE as f32) as usize;
        let len = (note.dur * SAMPLE_RATE as f32) as usize;
        let mut phase = 0.0f32;
        for i in 0..len {
            let t = i as f32 / SAMPLE_RATE as f32;
            let env = note_envelope(t, note.dur);
            let vibrato = if note.dur > 0.18 {
                1.0 + 0.003 * (std::f32::consts::TAU * 5.5 * t).sin()
            } else {
                1.0
            };
            phase += freq * vibrato / SAMPLE_RATE as f32;
            let sample = match note.voice {
                Voice::Lead => pulse(phase, 0.5) * 0.25,
                Voice::Bass => triangle(phase) * 0.20,
            };
            if start + i < buffer.len() {
                buffer[start + i] += sample * env;
            }
        }
    }

    soft_lowpass(&mut buffer, 8000.0);
    normalize(&mut buffer, 0.72);
    buffer
}

fn note_envelope(t: f32, dur: f32) -> f32 {
    let attack = 0.008;
    let release = 0.028;
    if t < attack {
        t / attack
    } else if t > dur - release {
        ((dur - t) / release).clamp(0.0, 1.0)
    } else {
        1.0
    }
}

fn pulse(phase: f32, duty: f32) -> f32 {
    if phase.fract() < duty {
        1.0
    } else {
        -1.0
    }
}

fn triangle(phase: f32) -> f32 {
    let p = phase.fract();
    4.0 * (p - 0.5).abs() - 1.0
}

fn soft_lowpass(buffer: &mut [f32], cutoff: f32) {
    let a = 1.0 - (-std::f32::consts::TAU * cutoff / SAMPLE_RATE as f32).exp();
    let mut y = 0.0f32;
    for sample in buffer.iter_mut() {
        y += a * (*sample - y);
        *sample = y;
    }
}

fn normalize(buffer: &mut [f32], peak: f32) {
    let max = buffer.iter().fold(0.0f32, |m, s| m.max(s.abs()));
    if max > 0.0 {
        let gain = peak / max;
        for s in buffer.iter_mut() {
            *s *= gain;
        }
    }
}

// ---------------------------------------------------------------------------
// Sound effects
// ---------------------------------------------------------------------------

fn add_at(buffer: &mut Vec<f32>, samples: &[f32], at_sec: f32) {
    let offset = (at_sec * SAMPLE_RATE as f32) as usize;
    if buffer.len() < offset + samples.len() {
        buffer.resize(offset + samples.len(), 0.0);
    }
    for (i, &s) in samples.iter().enumerate() {
        buffer[offset + i] += s;
    }
}

fn tone(freq: f32, dur: f32, duty: f32, gain: f32) -> Vec<f32> {
    let len = (dur * SAMPLE_RATE as f32) as usize;
    let mut out = Vec::with_capacity(len);
    let mut phase = 0.0f32;
    for i in 0..len {
        let t = i as f32 / SAMPLE_RATE as f32;
        let env = note_envelope(t, dur);
        phase += freq / SAMPLE_RATE as f32;
        out.push(pulse(phase, duty) * gain * env);
    }
    out
}

fn sweep(from: f32, to: f32, dur: f32, duty: f32, gain: f32) -> Vec<f32> {
    let len = (dur * SAMPLE_RATE as f32) as usize;
    let mut out = Vec::with_capacity(len);
    let mut phase = 0.0f32;
    for i in 0..len {
        let t = i as f32 / SAMPLE_RATE as f32;
        let k = t / dur;
        let freq = from + (to - from) * k;
        let env = (1.0 - k).powf(1.4);
        phase += freq / SAMPLE_RATE as f32;
        out.push(pulse(phase, duty) * gain * env);
    }
    out
}

fn noise(dur: f32, gain: f32, cutoff: f32, decay: f32, seed: u64) -> Vec<f32> {
    let len = (dur * SAMPLE_RATE as f32) as usize;
    let mut out = Vec::with_capacity(len);
    let mut state = seed | 1;
    let mut lp = 0.0f32;
    let a = 1.0 - (-std::f32::consts::TAU * cutoff / SAMPLE_RATE as f32).exp();
    for i in 0..len {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        let white = ((state >> 40) as f32 / (1u64 << 24) as f32) * 2.0 - 1.0;
        lp += a * (white - lp);
        let t = i as f32 / SAMPLE_RATE as f32;
        let env = (1.0 - (t / dur).clamp(0.0, 1.0)).powf(decay);
        out.push(lp * gain * env);
    }
    out
}

fn arpeggio(freqs: &[f32], step: f32, dur_each: f32, gain: f32) -> Vec<f32> {
    let mut out = Vec::new();
    for (i, &freq) in freqs.iter().enumerate() {
        add_at(&mut out, &tone(freq, dur_each, 0.5, gain), i as f32 * step);
    }
    out
}

fn sfx_definitions() -> Vec<(&'static str, Vec<f32>)> {
    let mut clips: Vec<(&'static str, Vec<f32>)> = Vec::new();

    // Piece movement: the classic short high pulse blip.
    clips.push(("move", sweep(1200.0, 880.0, 0.045, 0.5, 0.5)));
    // Rotation: a slightly lower two-step chirp.
    clips.push(("rotate", sweep(620.0, 980.0, 0.06, 0.25, 0.5)));
    // Lock: noise thud plus a low pulse.
    let mut lock = noise(0.08, 0.55, 900.0, 2.0, 0x1234_5678);
    add_at(&mut lock, &tone(90.0, 0.1, 0.5, 0.55), 0.0);
    clips.push(("lock", lock));
    // Hard drop: descending noise swoosh.
    let mut drop = sweep(900.0, 140.0, 0.14, 0.5, 0.45);
    add_at(&mut drop, &noise(0.12, 0.4, 1600.0, 2.5, 0x9ABC_DEF0), 0.01);
    clips.push(("drop", drop));
    // Hold: two-note blip.
    let mut hold = tone(523.0, 0.05, 0.5, 0.45);
    add_at(&mut hold, &tone(784.0, 0.05, 0.5, 0.45), 0.06);
    clips.push(("hold", hold));
    // Line clear: rising sweep and sparkle, NES style.
    let mut line = sweep(320.0, 1500.0, 0.3, 0.25, 0.5);
    add_at(
        &mut line,
        &noise(0.28, 0.28, 5000.0, 2.0, 0x0BAD_F00D),
        0.02,
    );
    clips.push(("line", line));
    // Tetris fanfare.
    let mut tetris = arpeggio(&[523.25, 659.25, 783.99, 1046.5], 0.09, 0.12, 0.4);
    add_at(&mut tetris, &noise(0.3, 0.3, 6000.0, 2.0, 0xFEED_1234), 0.0);
    add_at(&mut tetris, &tone(65.4, 0.4, 0.5, 0.4), 0.0);
    clips.push(("tetris", tetris));
    // Level up.
    clips.push((
        "level",
        arpeggio(&[440.0, 554.37, 659.25, 880.0], 0.08, 0.12, 0.4),
    ));
    // Game over: descending dirge.
    let mut over = arpeggio(&[440.0, 349.23, 293.66, 220.0], 0.22, 0.26, 0.4);
    add_at(&mut over, &tone(110.0, 0.9, 0.5, 0.25), 0.0);
    clips.push(("gameover", over));
    // UI blips.
    clips.push(("ui", tone(880.0, 0.05, 0.5, 0.4)));
    clips.push(("pause", tone(660.0, 0.08, 0.5, 0.4)));

    for (_, samples) in clips.iter_mut() {
        normalize(samples, 0.85);
    }
    clips
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wav_header_is_valid_pcm() {
        let samples = vec![0.0, 0.5, -0.5];
        let bytes = wav_bytes(&samples);
        assert_eq!(&bytes[0..4], b"RIFF");
        assert_eq!(&bytes[8..12], b"WAVE");
        assert_eq!(&bytes[12..16], b"fmt ");
        assert_eq!(u32::from_le_bytes(bytes[16..20].try_into().unwrap()), 16);
        assert_eq!(u16::from_le_bytes(bytes[20..22].try_into().unwrap()), 1);
        assert_eq!(u16::from_le_bytes(bytes[22..24].try_into().unwrap()), 1);
        assert_eq!(
            u32::from_le_bytes(bytes[24..28].try_into().unwrap()),
            SAMPLE_RATE
        );
        assert_eq!(bytes.len(), 44 + samples.len() * 2);
    }

    #[test]
    fn music_has_expected_loop_length() {
        let music = build_music();
        let expected = 25.6 * SAMPLE_RATE as f32;
        assert!(
            (music.len() as f32 - expected).abs() < 2.0,
            "music should loop after ~25.6s, got {} samples",
            music.len()
        );
        assert!(music.iter().any(|s| s.abs() > 0.1), "music must be audible");
        assert!(
            music.iter().all(|s| s.abs() <= 1.0),
            "samples must not clip"
        );
    }

    #[test]
    fn melody_starts_with_korobeiniki_notes() {
        let sixteenth = 60.0 / BPM / 4.0;
        let lead = parse_sequence(LEAD, sixteenth, Voice::Lead);
        let first = lead.first().expect("melody has notes");
        let freq = first.freq.unwrap();
        // Korobeiniki opens on E5 (659.25 Hz).
        assert!((freq - 659.25).abs() < 0.5, "first note was {freq}");
    }

    #[test]
    fn every_sfx_is_audible_and_bounded() {
        for (name, samples) in sfx_definitions() {
            assert!(!samples.is_empty(), "{name} is empty");
            assert!(samples.iter().any(|s| s.abs() > 0.2), "{name} is inaudible");
            assert!(samples.iter().all(|s| s.abs() <= 1.0), "{name} clips");
        }
    }
}
