use std::error::Error;
use std::sync::{Arc, Mutex};
use std::collections::HashMap;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{StreamConfig, Stream};

use crate::error::SynthError;
use crate::instrument::{Instrument, InstrumentSource, SampleData, SequenceElement};
use crate::track::{MelodyTrack, LoopPoint};
use crate::arrangement::{Arrangement, TrackOverrides};
use crate::effects::EffectsProcessor;

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
                        
                        // Apply fade in/out envelopes
                        let current_time = context.current_sample as f32 / sample_rate;
                        let total_length = context.arrangement.total_length;
                        let mut fade_mult = 1.0;
                        
                        if let Some(fade_in_dur) = context.arrangement.fade_in {
                            if current_time < fade_in_dur {
                                fade_mult *= current_time / fade_in_dur;
                            }
                        }
                        
                        if let Some(fade_out_dur) = context.arrangement.fade_out {
                            let fade_out_start = total_length - fade_out_dur;
                            if current_time > fade_out_start {
                                fade_mult *= (total_length - current_time) / fade_out_dur;
                            }
                        }
                        
                        let final_output = output * context.dynamic_params.master_volume * fade_mult;
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
            
            // Go through all sequence elements (notes, chords, rests)
            for element in &track.sequence {
                match element {
                    SequenceElement::Note(note) => {
                        let note_duration = note.duration * beat_duration;
                        let next_time = cumulative_time + note_duration;
                        
                        if track_time >= cumulative_time && track_time < next_time {
                            let time_in_note = track_time - cumulative_time;
                            let envelope = Self::calculate_envelope_static(time_in_note, note_duration, &track.instrument);
                            
                            // Apply pitch slide when specified
                            let mut pitch = note.pitch;
                            if let Some(slide_target) = note.slide_to {
                                let slide_progress = time_in_note / note_duration;
                                pitch = note.pitch * (1.0 - slide_progress) + slide_target * slide_progress;
                            }
                            
                            let sample = match &track.instrument.source {
                                InstrumentSource::Synthesized(waveform) => {
                                    let phase = (track_time * pitch * params.master_pitch) % 1.0;
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
                    SequenceElement::Chord(chord) => { // Handle chord playback
                        let chord_duration = chord.duration * beat_duration;
                        let next_time = cumulative_time + chord_duration;
                        
                        if track_time >= cumulative_time && track_time < next_time {
                            let time_in_note = track_time - cumulative_time;
                            let envelope = Self::calculate_envelope_static(time_in_note, chord_duration, &track.instrument);
                            
                            // Play all pitches in the chord simultaneously
                            for pitch in &chord.pitches {
                                let sample = match &track.instrument.source {
                                    InstrumentSource::Synthesized(waveform) => {
                                        let phase = (track_time * pitch * params.master_pitch) % 1.0;
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
                                output += sample * envelope * chord.velocity * volume / chord.pitches.len() as f32;
                            }
                            break;
                        }
                        
                        cumulative_time = next_time;
                    }
                    SequenceElement::Rest(duration) => { // handle rests
                        let rest_duration = duration * beat_duration;
                        cumulative_time += rest_duration;
                    }
                }
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
            if let Some(f) = &overrides.filter { t.instrument.effects.filter = Some(f.clone()); }

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

        // Apply fade in to beginning of buffer
        if let Some(fade_in_dur) = arrangement.fade_in {
            let fade_in_samples = (fade_in_dur * self.sample_rate) as usize;
            for i in 0..fade_in_samples.min(buffer.len()) {
                let fade_mult = i as f32 / fade_in_samples as f32;
                buffer[i] *= fade_mult;
            }
        }
        
        // Apply fade out to end of buffer
        if let Some(fade_out_dur) = arrangement.fade_out {
            let fade_out_samples = (fade_out_dur * self.sample_rate) as usize;
            let fade_start = buffer.len().saturating_sub(fade_out_samples);
            for i in fade_start..buffer.len() {
                let fade_mult = (buffer.len() - i) as f32 / fade_out_samples as f32;
                buffer[i] *= fade_mult;
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

        // Process all sequence elements (notes, chords, rests)
        for element in &track.sequence {
            match element {
                SequenceElement::Note(note) => {
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
                                
                                let mut pitch = note.pitch;
                                if let Some(slide_target) = note.slide_to {
                                    let slide_progress = time_in_note / note_duration_seconds;
                                    pitch = note.pitch * (1.0 - slide_progress) + slide_target * slide_progress;
                                }
                                
                                if let InstrumentSource::Synthesized(waveform) = &track.instrument.source {
                                    let output = waveform.generate_sample(phase);
                                    phase += pitch / self.sample_rate;
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
                SequenceElement::Chord(chord) => { // Synth chord
                    let chord_duration_seconds = chord.duration * beat_duration;
                    let chord_samples = (chord_duration_seconds * self.sample_rate) as usize;
                    
                    // Render each pitch in the chord
                    for pitch in &chord.pitches {
                        let mut phase = 0.0f32;
                        
                        for i in 0..chord_samples {
                            let sample_idx = start_sample + current_sample + i;
                            if sample_idx >= buffer.len() {
                                break;
                            }

                            let time_in_note = i as f32 / self.sample_rate;
                            let envelope = self.calculate_envelope(time_in_note, chord_duration_seconds, &track.instrument);
                            
                            if let InstrumentSource::Synthesized(waveform) = &track.instrument.source {
                                let output = waveform.generate_sample(phase);
                                phase += pitch / self.sample_rate;
                                if phase >= 1.0 {
                                    phase -= 1.0;
                                }
                                
                                buffer[sample_idx] += output * envelope * chord.velocity * track.instrument.volume / chord.pitches.len() as f32;
                            }
                        }
                    }
                    
                    current_sample += chord_samples;
                }
                SequenceElement::Rest(duration) => { // Skip forward for rest
                    let rest_duration_seconds = duration * beat_duration;
                    let rest_samples = (rest_duration_seconds * self.sample_rate) as usize;
                    current_sample += rest_samples;
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