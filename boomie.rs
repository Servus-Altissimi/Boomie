//  ______   _______  _______  _______ _________ _______ 
// (  ___ \ (  ___  )(  ___  |(       )\__   __/(  ____ \
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

use std::error::Error;
use std::fmt;
use std::sync::{Arc, Mutex};
use std::collections::{HashMap, VecDeque};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{StreamConfig, Stream};

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
    fn generate_sample(&self, phase: f32) -> f32 { // Phase should be in the range [0.0, 1.0)
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
pub struct ReverbParams {
    pub room_size: f32,
    pub damping: f32,
    pub wet: f32,
    pub width: f32,
}

impl Default for ReverbParams {
    fn default() -> Self {
        ReverbParams {
            room_size: 0.5,
            damping: 0.5,
            wet: 0.3,
            width: 1.0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct DelayParams {
    pub time: f32,
    pub feedback: f32,
    pub wet: f32,
}

impl Default for DelayParams {
    fn default() -> Self {
        DelayParams {
            time: 0.25,
            feedback: 0.4,
            wet: 0.3,
        }
    }
}

#[derive(Debug, Clone)]
pub struct DistortionParams {
    pub drive: f32,
    pub tone: f32,
    pub wet: f32,
}

impl Default for DistortionParams {
    fn default() -> Self {
        DistortionParams {
            drive: 2.0,
            tone: 0.7,
            wet: 0.5,
        }
    }
}

#[derive(Debug, Clone)]
pub struct EffectsChain {
    pub reverb: Option<ReverbParams>,
    pub delay: Option<DelayParams>,
    pub distortion: Option<DistortionParams>,
}

impl EffectsChain {
    pub fn has_any(&self) -> bool {
        self.reverb.is_some() || self.delay.is_some() || self.distortion.is_some()
    }
}


impl Default for EffectsChain {
    fn default() -> Self {
        EffectsChain {
            reverb: None,
            delay: None,
            distortion: None,
        }
    }
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
    pub effects: EffectsChain,
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
            effects: EffectsChain::default(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Note {
    pub pitch: f32,
    pub duration: f32,
    pub velocity: f32,
}


#[derive(Debug, Clone, Default)]
pub struct TrackOverrides {
    pub volume: Option<f32>,
    pub pitch: Option<f32>,
    pub tempo: Option<f32>,
    pub reverb: Option<ReverbParams>,
    pub delay: Option<DelayParams>,
    pub distortion: Option<DistortionParams>,
}

#[derive(Debug, Clone)]
pub struct LoopPoint {
    pub start: f32,
    pub end: f32,
}

#[derive(Debug, Clone)]
pub struct MelodyTrack {
    pub name: String,
    pub instrument: Instrument,
    pub notes: Vec<Note>,
    pub tempo: f32,
    pub length: f32,
    pub loop_point: Option<LoopPoint>,
}

impl MelodyTrack {
    pub fn from_mel(content: &str, sample_cache: &HashMap<String, SampleData>) -> Result<Self, SynthError> {
        let mut track = MelodyTrack {
            name: "melody".to_string(),
            instrument: Instrument::default(),
            notes: Vec::new(),
            tempo: 120.0,
            length: 0.0,
            loop_point: None,
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
            } else if let Some(v) = line.strip_prefix("loop:") {
                let parts: Vec<&str> = v.split(',').map(|s| s.trim()).collect();
                if parts.len() >= 2 {
                    track.loop_point = Some(LoopPoint {
                        start: parts[0].parse().unwrap_or(0.0),
                        end: parts[1].parse().unwrap_or(track.length),
                    });
                }
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
            } else if let Some(v) = line.strip_prefix("reverb:") {
                let parts: Vec<&str> = v.split(',').map(|s| s.trim()).collect();
                if parts.len() >= 4 {
                    track.instrument.effects.reverb = Some(ReverbParams {
                        room_size: parts[0].parse().unwrap_or(0.5),
                        damping: parts[1].parse().unwrap_or(0.5),
                        wet: parts[2].parse().unwrap_or(0.3),
                        width: parts[3].parse().unwrap_or(1.0),
                    });
                }
            } else if let Some(v) = line.strip_prefix("delay:") {
                let parts: Vec<&str> = v.split(',').map(|s| s.trim()).collect();
                if parts.len() >= 3 {
                    track.instrument.effects.delay = Some(DelayParams {
                        time: parts[0].parse().unwrap_or(0.25),
                        feedback: parts[1].parse().unwrap_or(0.4),
                        wet: parts[2].parse().unwrap_or(0.3),
                    });
                }
            } else if let Some(v) = line.strip_prefix("distortion:") {
                let parts: Vec<&str> = v.split(',').map(|s| s.trim()).collect();
                if parts.len() >= 3 {
                    track.instrument.effects.distortion = Some(DistortionParams {
                        drive: parts[0].parse().unwrap_or(2.0),
                        tone: parts[1].parse().unwrap_or(0.7),
                        wet: parts[2].parse().unwrap_or(0.5),
                    });
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
    pub tracks: Vec<(MelodyTrack, f32, TrackOverrides)>,  // TrackOverrides
    pub total_length: f32,
    pub loop_point: Option<LoopPoint>,
}

impl Arrangement {
    pub fn from_bmi(content: &str, mel_cache: &HashMap<String, MelodyTrack>) -> Result<Self, SynthError> {
        let mut arrangement = Arrangement {
            name: "song".to_string(),
            tracks: Vec::new(),
            total_length: 0.0,
            loop_point: None,
        };

        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with("//") {
                continue;
            }

            if let Some(value) = line.strip_prefix("name:") {
                arrangement.name = value.trim().to_string();
            } else if let Some(value) = line.strip_prefix("loop:") {
                let parts: Vec<&str> = value.split(',').map(|s| s.trim()).collect();
                if parts.len() >= 2 {
                    arrangement.loop_point = Some(LoopPoint {
                        start: parts[0].parse().unwrap_or(0.0),
                        end: parts[1].parse().unwrap_or(arrangement.total_length),
                    });
                }
            } else if let Some(value) = line.strip_prefix("track:") {
                let parts: Vec<&str> = value.split(',').map(|s| s.trim()).collect();
                if parts.len() >= 2 {
                    let mel_file = parts[0];
                    let start_time: f32 = parts[1].parse()
                        .map_err(|_| SynthError::ParseError("Invalid start time".to_string()))?;
                    
                    let mut overrides = TrackOverrides::default();
                    
                    for override_str in parts.iter().skip(2) {
                        if let Some((key, val)) = override_str.split_once('=') {
                            let key = key.trim();
                            let val = val.trim();
                            
                            match key {
                                "volume" | "vol" => {
                                    overrides.volume = val.parse().ok();
                                }
                                "pitch" => {
                                    overrides.pitch = val.parse().ok();
                                }
                                "tempo" => {
                                    overrides.tempo = val.parse().ok();
                                }
                                "reverb" => {
                                    let vals: Vec<&str> = val.split(':').collect();
                                    if vals.len() >= 4 {
                                        overrides.reverb = Some(ReverbParams {
                                            room_size: vals[0].parse().unwrap_or(0.5),
                                            damping: vals[1].parse().unwrap_or(0.5),
                                            wet: vals[2].parse().unwrap_or(0.3),
                                            width: vals[3].parse().unwrap_or(1.0),
                                        });
                                    }
                                }
                                "delay" => {
                                    let vals: Vec<&str> = val.split(':').collect();
                                    if vals.len() >= 3 {
                                        overrides.delay = Some(DelayParams {
                                            time: vals[0].parse().unwrap_or(0.25),
                                            feedback: vals[1].parse().unwrap_or(0.4),
                                            wet: vals[2].parse().unwrap_or(0.3),
                                        });
                                    }
                                }
                                "distortion" | "dist" => {
                                    let vals: Vec<&str> = val.split(':').collect();
                                    if vals.len() >= 3 {
                                        overrides.distortion = Some(DistortionParams {
                                            drive: vals[0].parse().unwrap_or(2.0),
                                            tone: vals[1].parse().unwrap_or(0.7),
                                            wet: vals[2].parse().unwrap_or(0.5),
                                        });
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                    
                    if let Some(track) = mel_cache.get(mel_file) {
                        let mut modified_track = track.clone();
                        
                        if overrides.tempo.is_some() {
                            modified_track.tempo = overrides.tempo.unwrap();
                        }
                        
                        arrangement.tracks.push((modified_track, start_time, overrides));
                        let end_time = start_time + track.length;
                        if end_time > arrangement.total_length {
                            arrangement.total_length = end_time;
                        }
                    } else {
                        eprintln!("Warning: Track not found in cache: \'{}\' Skipping track", mel_file);
                    }
                }
            }
        }

        // Return error only if the arrangement has no valid tracks
        if arrangement.tracks.is_empty() {
            return Err(SynthError::InvalidInstrument(
                "Arrangement has no valid tracks".to_string()
            ));
        }

        Ok(arrangement)
    }
}

pub struct EffectsProcessor {
    sample_rate: f32,
    comb_buffers: Vec<VecDeque<f32>>,
    comb_filter_state: Vec<f32>,
    allpass_buffers: Vec<VecDeque<f32>>,
    delay_buffer: VecDeque<f32>,
    lowpass_state: f32,
}

impl EffectsProcessor {
    pub fn new(sample_rate: f32) -> Self {
        let scale = sample_rate / 44100.0; 
        let comb_delays = vec![ // Freeverb design 
            (1116.0 * scale) as usize,
            (1188.0 * scale) as usize,
            (1277.0 * scale) as usize,
            (1356.0 * scale) as usize,
            (1422.0 * scale) as usize,
            (1491.0 * scale) as usize,
            (1557.0 * scale) as usize,
            (1617.0 * scale) as usize,
        ];

        let allpass_delays = vec![
            (556.0 * scale) as usize,
            (441.0 * scale) as usize,
            (341.0 * scale) as usize,
            (225.0 * scale) as usize,
        ];

        EffectsProcessor {
            sample_rate,
            comb_buffers: comb_delays.iter()
                .map(|&size| VecDeque::from(vec![0.0; size]))
                .collect(),
            comb_filter_state: vec![0.0; 8],
            allpass_buffers: allpass_delays.iter()
                .map(|&size| VecDeque::from(vec![0.0; size]))
                .collect(),
            delay_buffer: VecDeque::from(vec![0.0; (sample_rate * 2.0) as usize]),
            lowpass_state: 0.0,
        }
    }

    pub fn process(&mut self, input: f32, effects: &EffectsChain) -> f32 {
        let mut output = input;

        if let Some(dist) = &effects.distortion {
            output = self.apply_distortion(output, dist);
        }

        if let Some(delay) = &effects.delay {
            output = self.apply_delay(output, delay);
        }

        if let Some(reverb) = &effects.reverb {
            output = self.apply_reverb(output, reverb);
        }

        output
    }

    fn apply_distortion(&mut self, input: f32, params: &DistortionParams) -> f32 {
        let driven = input * params.drive;
        let distorted = if driven > 1.0 {
            2.0 / 3.0
        } else if driven < -1.0 {
            -2.0 / 3.0
        } else {
            driven - (driven.powi(3) / 3.0)
        };

        let alpha = 1.0 - params.tone;
        self.lowpass_state = self.lowpass_state * alpha + distorted * (1.0 - alpha);

        input * (1.0 - params.wet) + self.lowpass_state * params.wet
    }

    fn apply_delay(&mut self, input: f32, params: &DelayParams) -> f32 {
        let delay_samples = (params.time * self.sample_rate) as usize;
        let delay_samples = delay_samples.min(self.delay_buffer.len() - 1);

        let delayed = self.delay_buffer[delay_samples];

        Self::cycle_buffer(&mut self.delay_buffer, input + delayed * params.feedback);

        input * (1.0 - params.wet) + delayed * params.wet
    }

    fn apply_reverb(&mut self, input: f32, params: &ReverbParams) -> f32 {
        let mut output = 0.0;

        for i in 0..8 {
            let delayed = self.comb_buffers[i].back().copied().unwrap_or(0.0);
            
            self.comb_filter_state[i] = delayed * (1.0 - params.damping) + 
                                        self.comb_filter_state[i] * params.damping;
            
            let feedback = self.comb_filter_state[i] * params.room_size;
            
            Self::cycle_buffer(&mut self.comb_buffers[i], input + feedback);
            
            output += delayed;
        }

        output /= 8.0;

        for buffer in &mut self.allpass_buffers {
            let delayed = buffer.back().copied().unwrap_or(0.0);
            let new_val = output + delayed * 0.5;
            Self::cycle_buffer(buffer, new_val);
            output = delayed - output * 0.5;
        }

        input * (1.0 - params.wet) + output * params.wet
    }

    #[inline]
    fn cycle_buffer(buffer: &mut VecDeque<f32>, new_value: f32) {
        buffer.pop_back();
        buffer.push_front(new_value);
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PlaybackState {
    Stopped,
    Playing,
    Paused,
}

pub struct DynamicParameters {
    pub master_volume: f32,
    pub master_pitch: f32,
    pub track_volumes: HashMap<String, f32>,
    pub track_enabled: HashMap<String, bool>,
    pub crossfade_duration: f32,
}

impl Default for DynamicParameters {
    fn default() -> Self {
        DynamicParameters {
            master_volume: 1.0,
            master_pitch: 1.0,
            track_volumes: HashMap::new(),
            track_enabled: HashMap::new(),
            crossfade_duration: 1.0,
        }
    }
}

struct PlaybackContext {
    arrangement: Arrangement,
    current_sample: usize,
    state: PlaybackState,
    loop_enabled: bool,
    dynamic_params: DynamicParameters,
    param_interpolators: HashMap<String, f32>,
    crossfade_state: Option<CrossfadeState>,
}

struct CrossfadeState {
    target_arrangement: Arrangement,
    progress: f32,
    duration_samples: usize,
}

pub struct SynthEngine {
    mel_cache: HashMap<String, MelodyTrack>, // Cached melodies
    sample_cache: HashMap<String, SampleData>,
    stream_config: StreamConfig,
    sample_rate: f32,
    playback_context: Arc<Mutex<Option<PlaybackContext>>>,
    stream: Option<Stream>,
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
            playback_context: Arc::new(Mutex::new(None)),
            stream: None,
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

    pub fn play_arrangement(&mut self, arrangement: Arrangement) -> Result<(), SynthError> {
        self.stop();
        
        let mut context = PlaybackContext {
            arrangement,
            current_sample: 0,
            state: PlaybackState::Playing,
            loop_enabled: false,
            dynamic_params: DynamicParameters::default(),
            param_interpolators: HashMap::new(),
            crossfade_state: None,
        };
        
        for (track, _, _) in &context.arrangement.tracks {
            context.dynamic_params.track_enabled.insert(track.name.clone(), true);
            context.dynamic_params.track_volumes.insert(track.name.clone(), 1.0);
        }
        
        *self.playback_context.lock().unwrap() = Some(context);
        self.start_stream()?;
        
        Ok(())
    }

    pub fn crossfade_to(&mut self, new_arrangement: Arrangement, duration: f32) -> Result<(), SynthError> {
        {
            let mut ctx_lock = self.playback_context.lock().unwrap();

            if let Some(ctx) = ctx_lock.as_mut() {
                ctx.crossfade_state = Some(CrossfadeState {
                    target_arrangement: new_arrangement,
                    progress: 0.0,
                    duration_samples: (duration * self.sample_rate) as usize,
                });
                return Ok(());
            }
        }

        self.play_arrangement(new_arrangement)?;
        Ok(())
    }

    pub fn set_loop_enabled(&self, enabled: bool) {
        if let Some(ctx) = self.playback_context.lock().unwrap().as_mut() {
            ctx.loop_enabled = enabled;
        }
    }

    pub fn pause(&self) {
        if let Some(ctx) = self.playback_context.lock().unwrap().as_mut() {
            if ctx.state == PlaybackState::Playing {
                ctx.state = PlaybackState::Paused;
            }
        }
    }

    pub fn resume(&self) {
        if let Some(ctx) = self.playback_context.lock().unwrap().as_mut() {
            if ctx.state == PlaybackState::Paused {
                ctx.state = PlaybackState::Playing;
            }
        }
    }

    pub fn stop(&mut self) {
        if let Some(stream) = self.stream.take() {
            drop(stream);
        }
        *self.playback_context.lock().unwrap() = None;
    }

    pub fn set_master_volume(&self, volume: f32) {
        if let Some(ctx) = self.playback_context.lock().unwrap().as_mut() {
            ctx.dynamic_params.master_volume = volume.max(0.0).min(2.0);
        }
    }

    pub fn set_master_pitch(&self, pitch: f32) {
        if let Some(ctx) = self.playback_context.lock().unwrap().as_mut() {
            ctx.dynamic_params.master_pitch = pitch.max(0.5).min(2.0);
        }
    }

    pub fn set_track_enabled(&self, track_name: &str, enabled: bool) {
        if let Some(ctx) = self.playback_context.lock().unwrap().as_mut() {
            ctx.dynamic_params.track_enabled.insert(track_name.to_string(), enabled);
        }
    }

    pub fn set_track_volume(&self, track_name: &str, volume: f32) {
        if let Some(ctx) = self.playback_context.lock().unwrap().as_mut() {
            ctx.dynamic_params.track_volumes.insert(track_name.to_string(), volume.max(0.0).min(2.0));
        }
    }

    pub fn interpolate_track_volume(&self, track_name: &str, target: f32, duration: f32) {
        if let Some(ctx) = self.playback_context.lock().unwrap().as_mut() {
            let key = format!("vol_{}", track_name);
            ctx.param_interpolators.insert(key, duration);
            ctx.dynamic_params.track_volumes.insert(track_name.to_string(), target);
        }
    }

    pub fn get_playback_position(&self) -> f32 {
        if let Some(ctx) = self.playback_context.lock().unwrap().as_ref() {
            ctx.current_sample as f32 / self.sample_rate
        } else {
            0.0
        }
    }

    pub fn get_playback_state(&self) -> PlaybackState {
        if let Some(ctx) = self.playback_context.lock().unwrap().as_ref() {
            ctx.state
        } else {
            PlaybackState::Stopped
        }
    }

    fn start_stream(&mut self) -> Result<(), SynthError> {
        let host = cpal::default_host();
        let device = host.default_output_device()
            .ok_or_else(|| SynthError::AudioError("No output device".to_string()))?;
                    
            let config = self.stream_config.clone();
            let sample_rate = self.sample_rate;
            let ctx = Arc::clone(&self.playback_context);
                    
            let stream = device.build_output_stream(
                &config,
                move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                    let mut context_lock = ctx.lock().unwrap();
                    
                    if let Some(context) = context_lock.as_mut() {
                        if context.state != PlaybackState::Playing {
                            for sample in data.iter_mut() {
                                *sample = 0.0;
                            }
                            return;
                        }
                        
                        for frame in data.chunks_mut(config.channels as usize) {
                            let mut output = 0.0;
                            
                            // Current arrangement
                            output = Self::synthesize_single_sample(
                                &context.arrangement,
                                context.current_sample,
                                sample_rate,
                                &context.dynamic_params
                            );
                            
                            // Crossfade target
                            if let Some(ref mut crossfade) = context.crossfade_state {
                                let t = (crossfade.progress as f32) / (crossfade.duration_samples as f32);
                                
                                let target_sample = Self::synthesize_single_sample(
                                    &crossfade.target_arrangement,
                                    context.current_sample,
                                    sample_rate,
                                    &context.dynamic_params
                                );
                                
                                output = output * (1.0 - t) + target_sample * t;
                                crossfade.progress += 1.0;
                                
                                // Crossfade complete
                                if crossfade.progress >= crossfade.duration_samples as f32 {
                                    context.arrangement = crossfade.target_arrangement.clone();
                                    context.crossfade_state = None;
                                }
                            }
                            
                            context.current_sample += 1;
                            
                            // Loop/stop logic
                            if context.loop_enabled {
                                if let Some(ref loop_point) = context.arrangement.loop_point {
                                    let pos = context.current_sample as f32 / sample_rate;
                                    if pos >= loop_point.end {
                                        context.current_sample = (loop_point.start * sample_rate) as usize;
                                    }
                                } else {
                                    let total_samples = (context.arrangement.total_length * sample_rate) as usize;
                                    if context.current_sample >= total_samples {
                                        context.current_sample = 0;
                                    }
                                }
                            } else {
                                let total_samples = (context.arrangement.total_length * sample_rate) as usize;
                                if context.current_sample >= total_samples {
                                    context.state = PlaybackState::Stopped;
                                }
                            }
                            
                            let final_output = output * context.dynamic_params.master_volume;
                            for sample in frame.iter_mut() {
                                *sample = final_output;
                            }
                        }
                    } else {
                        for sample in data.iter_mut() {
                            *sample = 0.0;
                        }
                    }
                },
                |err| eprintln!("Stream error: {}", err),
                None
            ).map_err(|e| SynthError::AudioError(e.to_string()))?;

        stream.play().map_err(|e| SynthError::AudioError(e.to_string()))?;
        self.stream = Some(stream);
        
        Ok(())
    }

    fn synthesize_single_sample(
        arrangement: &Arrangement,
        sample_idx: usize,
        sample_rate: f32,
        params: &DynamicParameters
    ) -> f32 {
        let mut output = 0.0;
        let current_time = sample_idx as f32 / sample_rate;
        
        for (track, start_time, overrides) in &arrangement.tracks {
            let enabled = params.track_enabled.get(&track.name).copied().unwrap_or(true);
            if !enabled {
                continue;
            }
            
            let track_vol = params.track_volumes.get(&track.name).copied().unwrap_or(1.0);
            
            if current_time < *start_time {
                continue;
            }
            
            let track_time = current_time - start_time;
            
            let mut cumulative_time = 0.0;
            let beat_duration = 60.0 / track.tempo;
            
            for note in &track.notes {
                let note_duration = note.duration * beat_duration;
                let next_time = cumulative_time + note_duration;
                
                if track_time >= cumulative_time && track_time < next_time {
                    let time_in_note = track_time - cumulative_time;
                    let envelope = Self::calculate_envelope_static(time_in_note, note_duration, &track.instrument);
                    
                    let sample = match &track.instrument.source {
                        InstrumentSource::Synthesized(waveform) => {
                            let phase = (track_time * note.pitch * params.master_pitch) % 1.0;
                            waveform.generate_sample(phase)
                        }
                        InstrumentSource::Sample(sample_data) => {
                            Self::interpolate_sample(
                                sample_data,
                                time_in_note,
                                track.instrument.pitch * params.master_pitch
                            )
                        }
                    };
                    
                    let volume = track.instrument.volume * overrides.volume.unwrap_or(1.0) * track_vol;
                    output += sample * envelope * note.velocity * volume;
                    break;
                }
                
                cumulative_time = next_time;
            }
        }
        
        output
    }
    
    pub fn synthesize_arrangement(&self, arrangement: &Arrangement) -> Result<Vec<f32>, SynthError> {
        self.synthesize_arrangement_private(arrangement, &DynamicParameters::default())
    }

    fn synthesize_arrangement_private(
        &self,
        arrangement: &Arrangement,
        params: &DynamicParameters,
    ) -> Result<Vec<f32>, SynthError> {
        let total_samples = (arrangement.total_length * self.sample_rate) as usize;
        let mut buffer = vec![0.0; total_samples];
        let chunk_size = 1024;

        for (track, start_time, overrides) in &arrangement.tracks {
            let enabled = params.track_enabled.get(&track.name).copied().unwrap_or(true);
            if !enabled {
                continue;
            }

            let track_vol = params.track_volumes.get(&track.name).copied().unwrap_or(1.0);
            let start_sample = (start_time * self.sample_rate) as usize;

            let mut t = track.clone();

            // Apply overrides
            if let Some(v) = overrides.volume { t.instrument.volume = v; }
            if let Some(p) = overrides.pitch { t.instrument.pitch = p * params.master_pitch; }
            if let Some(tm) = overrides.tempo { t.tempo = tm; }
            if let Some(r) = &overrides.reverb { t.instrument.effects.reverb = Some(r.clone()); }
            if let Some(d) = &overrides.delay { t.instrument.effects.delay = Some(d.clone()); }
            if let Some(x) = &overrides.distortion { t.instrument.effects.distortion = Some(x.clone()); }

            t.instrument.volume *= track_vol;

            let track_total_samples = (t.length * self.sample_rate) as usize;
            let mut fx = if t.instrument.effects.has_any() {
                Some(EffectsProcessor::new(self.sample_rate))
            } else {
                None
            };

            let mut sample_offset = 0;
            while sample_offset < track_total_samples {
                let current_chunk_size = (chunk_size).min(track_total_samples - sample_offset);
                let mut chunk_buf = vec![0.0; current_chunk_size];

                self.synthesize_track_into(&mut chunk_buf, &t, sample_offset);

                if let Some(fx_processor) = &mut fx {
                    for s in chunk_buf.iter_mut() {
                        *s = fx_processor.process(*s, &t.instrument.effects);
                    }
                }

                // Mix chunk into main buffer
                for (i, &s) in chunk_buf.iter().enumerate() {
                    if let Some(dst) = buffer.get_mut(start_sample + sample_offset + i) {
                        *dst += s * params.master_volume;
                    }
                }

                sample_offset += current_chunk_size;
            }
        }

        // Normalize
        if let Some(max) = buffer.iter().map(|v| v.abs()).max_by(|a,b| a.partial_cmp(b).unwrap()) {
            if max > 1.0 { buffer.iter_mut().for_each(|s| *s /= max); }
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
                        
                        let sample_value = Self::interpolate_sample(
                            sample_data,
                            time_in_note,
                            pitch_adjusted_rate
                        );
                        
                        buffer[sample_idx] += sample_value * envelope * note.velocity * track.instrument.volume;
                    }
                    
                    current_sample += output_len; // Continue by the time the sample took to play

                }
            }
        }
    }

    #[inline]
    fn interpolate_sample(sample_data: &SampleData, time_in_note: f32, pitch_rate: f32) -> f32 {
        let src_pos = time_in_note * sample_data.sample_rate as f32 * pitch_rate;
        let src_idx = src_pos as usize;
        
        if src_idx >= sample_data.samples.len() {
            return 0.0;
        }
        
        // Linear interpolation
        if src_idx < sample_data.samples.len() - 1 {
            let frac = src_pos - src_idx as f32;
            let s1 = sample_data.samples[src_idx];
            let s2 = sample_data.samples[src_idx + 1];
            s1 * (1.0 - frac) + s2 * frac
        } else {
            sample_data.samples[src_idx]
        }
    }

    // Generate ADSR envelope value at a set time
    fn calculate_envelope(&self, time: f32, duration: f32, instr: &Instrument) -> f32 {
        Self::calculate_envelope_static(time, duration, instr)
    }

    fn calculate_envelope_static(time: f32, duration: f32, instr: &Instrument) -> f32 {
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
