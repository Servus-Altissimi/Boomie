//  ______   _______  _______  _______ _________ _______ 
// (  ___ \ (  ___  )(  ___  )(       )\__   __/(  ____ \
// | (   ) )| (   ) || (   ) || () () |   ) (   | (    \/
// | (__/ / | |   | || |   | || || || |   | |   | (__    
// |  __ (  | |   | || |   | || |(_)| |   | |   |  __)   
// | (  \ \ | |   | || |   | || |   | |   | |   | (      
// | )___) )| (___) || (___) || )   ( |___) (___| (____/\
// |/ \___/ (_______)(_______)|/     \|\_______/(_______/
               
// Copyright 2025 Servus Altissimi (Pseudonym)

// Permission is hereby granted, free of charge, to any person obtaining a copy of this software and associated documentation files (the "Software"), to deal in the Software without restriction, including without limitation the rights to use, copy, modify, merge, publish, distribute, sublicense, and/or sell copies of the Software, and to permit persons to whom the Software is furnished to do so, subject to the following conditions:
// The above copyright notice and this permission notice shall be included in all copies or substantial portions of the Software.
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.


use std::collections::HashMap;
use std::error::Error;
use std::fmt;
use std::sync::Arc;
use cpal::traits::{DeviceTrait, HostTrait};
use cpal::StreamConfig;

impl std::error::Error for SynthError {}

#[derive(Debug, Clone)]
pub enum SynthError {
    ParseError(String),
    FileError(String),
    AudioError(String),
    InvalidInstrument(String),
}

impl fmt::Display for SynthError { // TODO, expand
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            SynthError::ParseError(msg) => write!(f, "Parsing Error: {}", msg),
            SynthError::FileError(msg) => write!(f, "File Error: {}", msg),
            SynthError::AudioError(msg) => write!(f, "Audio Error: {}", msg),
            SynthError::InvalidInstrument(msg) => write!(f, "Invalid Instrument Error: {}", msg),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum WaveformType {
    Sine,
    Square,
    Triangle,
    Sawtooth,
    Noise,
}

impl WaveformType {
    fn generate_sample(&self, phase: f32) -> f32 { /// Phase should be in the range [0.0, 1.0)
        match self {
            WaveformType::Sine => (phase * std::f32::consts::TAU).sin(),
            WaveformType::Square => if (phase * 2.0) % 1.0 < 0.5 { 1.0 } else { -1.0 },
            WaveformType::Sawtooth => (phase * 2.0) % 1.0 * 2.0 - 1.0,
            WaveformType::Noise => fastrand::f32() * 2.0 - 1.0, // WaveformType::Noise => (((phase * 1235.647).sin() * 43758.5453).fract() * 2.0 - 1.0), <- possible replacement
            WaveformType::Triangle => {
                let p = (phase * 2.0) % 1.0;
                if p < 0.5 { p * 4.0 - 1.0 } else { 3.0 - p * 4.0 }
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct SampleData {
    pub samples: Arc<Vec<f32>>,
    pub sample_rate: u32,
}

#[derive(Debug, Clone)]
pub enum InstrumentSource {
    Synthesized(WaveformType),
    Sample(SampleData),
}

#[derive(Debug, Clone)]
pub struct Instrument {
    pub name: String,
    pub source: InstrumentSource,
    pub attack: f32, // ADSR envelope parameters
    pub decay: f32,
    pub sustain: f32,
    pub release: f32,
    pub volume: f32,
    pub pitch: f32,
}

impl Default for Instrument {
    fn default() -> Self {
        Instrument {
            name: "Boomie".to_string(),
            source: InstrumentSource::Synthesized(WaveformType::Sine),
            attack: 0.01,
            decay: 0.1,
            sustain: 0.8,
            release: 0.2,
            volume: 0.5,
            pitch: 1.0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Note {
    pub pitch: f32,
    pub duration: f32,
    pub velocity: f32,
}

#[derive(Debug, Clone)]
pub struct MelodyTrack {
    pub name: String,
    pub instrument: Instrument,
    pub notes: Vec<Note>,
    pub tempo: f32,
    pub length: f32,
}

impl MelodyTrack {
    pub fn from_mel(content: &str, sample_cache: &HashMap<String, SampleData>) -> Result<Self, SynthError> {
        let mut track = MelodyTrack {
            name: "melody".to_string(),
            instrument: Instrument::default(),
            notes: Vec::new(),
            tempo: 120.0,
            length: 0.0,
        };

        macro_rules! parse_field {
            ($line:expr, $prefix:expr, $field:expr) => {
                if let Some(v) = $line.strip_prefix($prefix) {
                    $field = v.trim().parse()
                        .map_err(|_| SynthError::ParseError(format!("Invalid {}", $prefix)))?;
                    continue;
                }
            };
        }

        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with("//") { continue; } // Comments (//) & empty lines 

            if let Some(v) = line.strip_prefix("name:") {
                track.name = v.trim().to_string();
            } else if let Some(v) = line.strip_prefix("sample:") {
                track.instrument.source = InstrumentSource::Sample(
                    sample_cache.get(v.trim())
                        .ok_or_else(|| SynthError::InvalidInstrument(format!("Sample not found: {}", v.trim())))?
                        .clone()
                );
            } else if let Some(v) = line.strip_prefix("waveform:") {
                track.instrument.source = InstrumentSource::Synthesized(match v.trim().to_lowercase().as_str() {
                    "sine" => WaveformType::Sine,
                    "square" => WaveformType::Square,
                    "triangle" => WaveformType::Triangle,
                    "sawtooth" => WaveformType::Sawtooth,
                    "noise" => WaveformType::Noise,
                    _ => return Err(SynthError::ParseError("Unknown Waveform".to_string())),
                });
            } else if let Some(v) = line.strip_prefix("note:") {
                let parts: Vec<&str> = v.split(',').map(|s| s.trim()).collect();
                if parts.len() >= 3 {
                    let pitch = parse_note(parts[0])?;
                    let duration: f32 = parts[1].parse()
                        .map_err(|_| SynthError::ParseError("Invalid Duration".to_string()))?;
                    let velocity: f32 = parts[2].split("//").next().unwrap_or("0").trim().parse()
                        .map_err(|_| SynthError::ParseError("Invalid Velocity".to_string()))?;
                    
                    track.notes.push(Note { pitch, duration, velocity });
                    track.length += duration;
                }
            } else {
                parse_field!(line, "tempo:", track.tempo);
                parse_field!(line, "volume:", track.instrument.volume);
                parse_field!(line, "attack:", track.instrument.attack);
                parse_field!(line, "decay:", track.instrument.decay);
                parse_field!(line, "sustain:", track.instrument.sustain);
                parse_field!(line, "release:", track.instrument.release);
                parse_field!(line, "pitch:", track.instrument.pitch);
            }
        }

        Ok(track)
    }
}

#[derive(Debug, Clone)]
pub struct Arrangement {
    pub name: String,
    pub tracks: Vec<(MelodyTrack, f32)>,
    pub total_length: f32,
}

impl Arrangement {
    pub fn from_bmi(content: &str, mel_cache: &HashMap<String, MelodyTrack>) -> Result<Self, SynthError> {
        let mut arrangement = Arrangement {
            name: "song".to_string(),
            tracks: Vec::new(),
            total_length: 0.0,
        };

        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with("//") {
                continue;
            }

            if let Some(value) = line.strip_prefix("name:") {
                arrangement.name = value.trim().to_string();
            } else if let Some(value) = line.strip_prefix("track:") {
                let parts: Vec<&str> = value.split(',').map(|s| s.trim()).collect();
                if parts.len() >= 2 {
                    let mel_file = parts[0];
                    let start_time: f32 = parts[1].parse()
                        .map_err(|_| SynthError::ParseError("Invalid start time".to_string()))?;
                    
                    if let Some(track) = mel_cache.get(mel_file) {
                        arrangement.tracks.push((track.clone(), start_time));
                        let end_time = start_time + track.length;
                        if end_time > arrangement.total_length {
                            arrangement.total_length = end_time;
                        }
                    } else {
                        return Err(SynthError::InvalidInstrument(
                            format!("Track not found: {}", mel_file)
                        ));
                    }
                }
            }
        }

        Ok(arrangement)
    }
}

pub struct SynthEngine {
    mel_cache: HashMap<String, MelodyTrack>, // Cached melodies
    sample_cache: HashMap<String, SampleData>,
    stream_config: StreamConfig,
    sample_rate: f32,
}

impl SynthEngine {
    pub fn new() -> Result<Self, SynthError> {
        let host = cpal::default_host();
        let device = host.default_output_device()
            .ok_or_else(|| SynthError::AudioError("No output device found".to_string()));
        let config = device?.default_output_config()
            .map_err(|e| SynthError::AudioError(e.to_string()))?;
        let stream_config = config.config();
        
        Ok(SynthEngine {
            mel_cache: HashMap::new(),
            sample_cache: HashMap::new(),
            stream_config: stream_config.clone(),
            sample_rate: stream_config.sample_rate.0 as f32,
        })
    }

    pub fn get_sample_cache(&self) -> &HashMap<String, SampleData> {
        &self.sample_cache
    }

    /// Load a .wav file into the sample cache
    pub fn load_sample(&mut self, name: &str, path: &str) -> Result<(), Box<dyn Error>> {
        let data = std::fs::read(path)?;
        let cursor = std::io::Cursor::new(data);
        let mut reader = hound::WavReader::new(cursor)?;
        let spec = reader.spec();
        
        println!("Loading sample \'{}\': {} Hz, {} channels", name, spec.sample_rate, spec.channels);
        println!("Output sample rate: {} Hz", self.sample_rate);
        
        let samples: Result<Vec<f32>, _> = reader.samples::<i16>()
            .map(|r| r.map(|s| s as f32 / 32768.0)) // i16 audio samples range from −32768 to 32767
            .collect();
        
        let sample_data = SampleData {
            samples: Arc::new(samples?),
            sample_rate: spec.sample_rate,
        };
        
        self.sample_cache.insert(name.to_string(), sample_data);
        Ok(())
    }

    pub fn load_melody(&mut self, name: &str, path: &str) -> Result<(), Box<dyn Error>> {
        let content = std::fs::read_to_string(path)?;
        let track = MelodyTrack::from_mel(&content, &self.sample_cache)?;
        self.mel_cache.insert(name.to_string(), track);
        Ok(())
    }

    pub fn load_arrangement(&self, path: &str) -> Result<Arrangement, SynthError> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| SynthError::FileError(e.to_string()))?;
        Arrangement::from_bmi(&content, &self.mel_cache)
    }

    pub fn synthesize_arrangement(&self, arrangement: &Arrangement) -> Result<Vec<f32>, SynthError> {
        let total_samples = (arrangement.total_length * self.sample_rate) as usize;
        let mut buffer = vec![0.0; total_samples];

        for (track, start_time) in &arrangement.tracks {
            let start_sample = (start_time * self.sample_rate) as usize;
            self.synthesize_track_into(&mut buffer, track, start_sample);
        }

        if let Some(max) = buffer.iter().map(|v| v.abs()).max_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal)) {
            if max > 1.0 {
                for sample in &mut buffer {
                    *sample /= max;
                }
            }
        }

        Ok(buffer)
    }

    fn synthesize_track_into(&self, buffer: &mut [f32], track: &MelodyTrack, start_sample: usize) {
        let mut current_sample = 0usize;
        let beat_duration = 60.0 / track.tempo;

        for note in &track.notes {
            let note_duration_seconds = note.duration * beat_duration;
            
            match &track.instrument.source {
                InstrumentSource::Synthesized(_) => {
                    let note_samples = (note_duration_seconds * self.sample_rate) as usize;
                    let mut phase = 0.0f32;
                    
                    for i in 0..note_samples {
                        let sample_idx = start_sample + current_sample + i;
                        if sample_idx >= buffer.len() {
                            break;
                        }

                        let time_in_note = i as f32 / self.sample_rate;
                        let envelope = self.calculate_envelope(time_in_note, note_duration_seconds, &track.instrument);
                        
                        if let InstrumentSource::Synthesized(waveform) = &track.instrument.source {
                            let output = waveform.generate_sample(phase);
                            phase += note.pitch / self.sample_rate;
                            if phase >= 1.0 {
                                phase -= 1.0;
                            }
                            
                            buffer[sample_idx] += output * envelope * note.velocity * track.instrument.volume;
                        }
                    }
                    
                    current_sample += note_samples;
                }
                
                InstrumentSource::Sample(sample_data) => {  // hell
                    let pitch_adjusted_rate = track.instrument.pitch;
                    let sample_len = sample_data.samples.len();
                    
                    let output_len = (sample_len as f32 / pitch_adjusted_rate) as usize;
                    let actual_duration = output_len as f32 / self.sample_rate;
                                        
                    for i in 0..output_len {
                        let sample_idx = start_sample + current_sample + i;
                        if sample_idx >= buffer.len() {
                            break;
                        }

                        let time_in_note = i as f32 / self.sample_rate;
                        let envelope = self.calculate_envelope(time_in_note, actual_duration, &track.instrument);
                        
                        let src_pos = i as f32 * pitch_adjusted_rate;
                        let src_idx = src_pos as usize;
                        let frac = src_pos - src_idx as f32;
                        
                        // Linear interpolation
                        let sample_value = if src_idx < sample_len - 1 {
                            let s1 = sample_data.samples[src_idx];
                            let s2 = sample_data.samples[src_idx + 1];
                            s1 * (1.0 - frac) + s2 * frac
                        } else if src_idx < sample_len {
                            sample_data.samples[src_idx]
                        } else {
                            0.0
                        };
                        
                        buffer[sample_idx] += sample_value * envelope * note.velocity * track.instrument.volume;
                    }
                    
                    current_sample += output_len; // Continue by the time the sample took to play

                }
            }
        }
    }

    fn calculate_envelope(&self, time: f32, duration: f32, instr: &Instrument) -> f32 { /// Generate ADSR envelope value at a set time
        let attack_end = instr.attack;
        let decay_end = attack_end + instr.decay;
        let release_start = duration - instr.release;

        // ramp, normalize, fade
        if time < attack_end {
            time / attack_end
        } else if time < decay_end {
            let decay_progress = (time - attack_end) / instr.decay;
            1.0 - decay_progress * (1.0 - instr.sustain)
        } else if time < release_start {
            instr.sustain
        } else {
            let release_progress = (time - release_start) / instr.release;
            instr.sustain * (1.0 - release_progress)
        }
    }
}

fn parse_note(note_str: &str) -> Result<f32, SynthError> { // https://en.wikipedia.org/wiki/Piano_key_frequencies
    let note_str = note_str.to_uppercase();
    let mut freq = match note_str.chars().next() {
        Some('C') => 16.35,
        Some('D') => 18.35,
        Some('E') => 20.60,
        Some('F') => 21.83,
        Some('G') => 24.50,
        Some('A') => 27.50,
        Some('B') => 30.87,
        _ => return Err(SynthError::ParseError("Invalid note".to_string())),
    };

    let second_char = note_str.chars().nth(1);
    let has_accidental = matches!(second_char, Some('#') | Some('S') | Some('B') | Some('F'));
    
    if has_accidental {
        match second_char {
            Some('#') | Some('S') => freq *= 1.059463,
            Some('B') | Some('F') => freq *= 0.943874,
            _ => {}
        }
    }

    let octave_start = if has_accidental { 2 } else { 1 };
    let octave_str = note_str[octave_start..].trim();

    if !octave_str.is_empty() {
        if let Ok(octave) = octave_str.parse::<i32>() {
            freq *= 2.0_f32.powi(octave);
        }
    }

    Ok(freq)
}
