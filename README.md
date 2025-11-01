# Boomie

```
 ______   _______  _______  _______ _________ _______ 
(  ___ \ (  ___  )(  ___  |(       )\__   __/(  ____ \
| (   ) )| (   ) || (   ) || () () |   ) (   | (    \/
| (__/ / | |   | || |   | || || || |   | |   | (__    
|  __ (  | |   | || |   | || |(_)| |   | |   |  __)   
| (  \ \ | |   | || |   | || |   | |   | |   | (      
| )___) )| (___) || (___) || )   ( |___) (___| (____/\
|/ \___/ (_______)(_______)|/     \|\_______/(_______/

```

[![Crates.io](https://img.shields.io/crates/v/Boomie.svg)](https://crates.io/crates/Boomie)

Real-time music synthesizer and composition engine written with dynamic playback control and advanced audio effects. Designed for on the fly compositing.

## Why
Boomie is made for the [Liefde.](https://github.com/servus-altissimi/Liefde.) game engine. The main reason for including a music synthesizer in a game engine is to enable dynamic control over the soundtrack. Of course this module can be used outside the context of a videogame, which is why I released it as a seperate crate. 

## Features

### Audio Synthesis
- **Waveform types**: Sine, Square, Triangle, Sawtooth, and Noise
- **Sample based playback**: Load and play WAV files with pitch adjustment and interpolation
- **ADSR envelope shaping**: Full Attack, Decay, Sustain, Release control per instrument
- **Real-time synthesis**: Low-latency audio output using `cpal`
- **Chord support**: Play multiple notes at once
- **Pitch slides**: Smooth pitch transitions between individual notes
- **Per-note parameters**: Individual pan and slide control for each note

### Effects Processing
- **Reverb**: Freeverb based algorithm with room size, damping, wet/dry mix, and stereo width controls
- **Delay**: Configurable delay time, feedback, and wet/dry mix with feedback loop
- **Distortion**: Waveshaping distortion with drive, tone control (lowpass filtering), and wet/dry mix
- **Filters**: Biquad filters supporting lowpass, highpass, and bandpass modes with cutoff and resonance control
- **Effects chain**: Process audio through multiple effects in sequence

### Dynamic Playback Control
- **Real-time parameter adjustment**: Change volume, pitch, and track states during playback
- **Crossfading**: Smooth transitions between different arrangements
- **Track muting**: Enable/disable individual tracks on the fly
- **Looping**: Support for arrangement level and track level loop points
- **Fade in/out**: Automatic fade envelopes for arrangement start and end
- **Master controls**: Global volume and pitch adjustment
- **Parameter interpolation**: Gradual volume changes over time

## Installation

Add Boomie to your `Cargo.toml`:

```toml
[dependencies]
Boomie = "0.1.0"
```

Or add via cargo:

```bash
cargo add Boomie
```

## Example

```rust
use boomie::*;
use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    let mut engine = SynthEngine::new()?;
    
    // Load samples
    engine.load_sample("kick", "samples/kick.wav")?;
        
    // Play arrangement
    let arrangement = engine.load_arrangement("songs/song.bmi")?;
    engine.play_arrangement(arrangement)?;
    engine.set_loop_enabled(true);
    
    engine.set_master_volume(0.8);
    engine.set_track_volume("bass", 1.2);
    std::thread::sleep(std::time::Duration::from_secs(30));
    engine.stop();
    
    Ok(())
}
```

## API

### Core Functions

| Function | Description |
|----------|-------------|
| `SynthEngine::new()` | Create a new synthesizer engine with default audio device |
| `load_sample(name, path)` | Load a `.wav` file into the sample cache |
| `load_melody(name, path)` | Parse and cache a `.mel` file |
| `load_arrangement(path)` | Load a `.bmi` arrangement file |
| `get_sample_cache()` | Get reference to loaded samples |
| `play_arrangement(arrangement)` | Start playback of an arrangement |
| `stop()` | Stop playback and clean up audio stream |
| `pause()` | Pause playback without stopping |
| `resume()` | Resume paused playback |
| `synthesize_arrangement(arrangement)` | Render arrangement to audio buffer |

### Playback Control

| Function | Description |
|----------|-------------|
| `set_loop_enabled(enabled)` | Enable/disable looping |
| `crossfade_to(arrangement, duration)` | Smoothly transition to new arrangement |
| `get_playback_position()` | Get current playback time in seconds |
| `get_playback_state()` | Get current state: `Playing`, `Paused`, or `Stopped` |

### Dynamic Parameters

| Function | Description | Range |
|----------|-------------|-------|
| `set_master_volume(volume)` | Set global volume | 0.0-2.0 |
| `set_master_pitch(pitch)` | Set global pitch multiplier | 0.5-2.0 |
| `set_track_enabled(name, enabled)` | Toggle a specific track | boolean |
| `set_track_volume(name, volume)` | Set track volume | 0.0-2.0 |
| `interpolate_track_volume(name, target, duration)` | Gradual volume change over time | target: 0.0-2.0, duration: seconds |

## File Format Reference

### Melody File (`.mel`)

#### Metadata

| Parameter | Description | Default |
|-----------|-------------|---------|
| `name:` | Track name (used for identification) | `"melody"` |
| `tempo:` | BPM | `120` |
| `time_sig:` | Time signature as `numerator/denominator` | `4/4` |
| `swing:` | Swing feel | `0.0` (straight) |
| `loop:` | Loop points in seconds: `start, end` | none |

#### Instrument Configuration

| Parameter | Description | Values/Range |
|-----------|-------------|--------------|
| `waveform:` | Synthesized waveform type | `sine`, `square`, `triangle`, `sawtooth`, `noise` |
| `sample:` | Reference to loaded sample by name | sample name string |
| `volume:` | Base amplitude | 0.0-1.0+ |
| `pitch:` | Pitch multiplier | any float > 0 |
| `pan:` | Stereo position | -1.0 (left) to 1.0 (right) |
| `detune:` | Pitch offset in cents | any float |

#### ADSR Envelope

| Parameter | Description | Range |
|-----------|-------------|-------|
| `attack:` | Attack time in seconds | 0.0+ |
| `decay:` | Decay time in seconds | 0.0+ |
| `sustain:` | Sustain level | 0.0-1.0 |
| `release:` | Release time in seconds | 0.0+ |

#### Sequence Elements

**Notes:**
```
note: PITCH, DURATION, VELOCITY [, PARAMS...]
```

| Parameter | Description | Example |
|-----------|-------------|---------|
| `PITCH` | Note name | `C4`, `D#5`, `Gb3` |
| `DURATION` | Length in beats | `1.0`, `0.5`, `2.0` |
| `VELOCITY` | Note volume | `0.8` (0.0-1.0) |
| `pan=` | Override stereo position | `pan=0.5` |
| `slide=` | Pitch slide target note | `slide=E4` |

**Chords:**
```
chord: NOTE1+NOTE2+NOTE3, DURATION, VELOCITY
```

| Component | Description |
|-----------|-------------|
| Notes | Multiple notes separated by `+` |
| Duration | Length in beats (applies to entire chord) |
| Velocity | Volume (applies to entire chord) |

**Rests:**
```
rest: DURATION
```
Silence for specified duration in beats.

#### Effects

| Effect | Syntax | Parameters |
|--------|--------|------------|
| Filter | `filter: TYPE, CUTOFF, RESONANCE` | Type: `lowpass`/`lp`, `highpass`/`hp`, `bandpass`/`bp`<br>Cutoff: Hz<br>Resonance: Q factor (0.1-10.0) |
| Reverb | `reverb: ROOM_SIZE, DAMPING, WET, WIDTH` | All parameters: 0.0-1.0 |
| Delay | `delay: TIME, FEEDBACK, WET` | Time: seconds<br>Feedback: 0.0-1.0<br>Wet: 0.0-1.0 |
| Distortion | `distortion: DRIVE, TONE, WET` | Drive: 1.0+<br>Tone: 0.0-1.0<br>Wet: 0.0-1.0 |

#### Example

```
name: bass
tempo: 120
waveform: sawtooth
volume: 0.9
attack: 0.01
decay: 0.2
sustain: 0.7
release: 0.3
pan: -0.3

note: C2, 1.0, 0.8
note: E2, 0.5, 0.9, slide=G2
chord: C2+E2+G2, 2.0, 0.7
rest: 0.5

filter: lowpass, 800, 0.5
reverb: 0.3, 0.4, 0.2, 0.9
```

### Bundles Music Index files/Arrangements (`.bmi`)

#### Metadata

| Parameter | Description | Default |
|-----------|-------------|---------|
| `name:` | Arrangement name | `"song"` |
| `master_tempo:` | Override tempo for all tracks | none |
| `fade_in:` | Fade in duration in seconds | none |
| `fade_out:` | Fade out duration in seconds | none |
| `loop:` | Arrangement loop points: `start, end` | none |

#### Tracks

```
track: MELODY_FILE, START_TIME [, OVERRIDES...]
```

| Component | Description |
|-----------|-------------|
| `MELODY_FILE` | Name of cached melody file |
| `START_TIME` | When track begins (seconds) |
| `OVERRIDES` | Optional parameter overrides |

#### Track Override Parameters

| Override | Syntax | Description |
|----------|--------|-------------|
| Volume | `volume=0.8` or `vol=0.8` | Track volume multiplier |
| Pitch | `pitch=1.2` | Pitch multiplier |
| Tempo | `tempo=140` | Override track tempo |
| Pan | `pan=0.5` | Override pan position |
| Filter | `filter=TYPE:CUTOFF:RESONANCE` | Add/override filter |
| Reverb | `reverb=ROOM:DAMP:WET:WIDTH` | Add/override reverb |
| Delay | `delay=TIME:FEEDBACK:WET` | Add/override delay |
| Distortion | `distortion=DRIVE:TONE:WET` or `dist=...` | Add/override distortion |

#### Example

```
name: Song
master_tempo: 120
fade_in: 2.0
fade_out: 3.0

track: bass.mel, 0.0, volume=1.2
track: melody.mel, 2.0, pitch=1.0, reverb=0.6:0.5:0.3:1.0
track: drums.mel, 4.0, filter=lowpass:800:0.5
track: kick.mel, 8.0, dist=3.0:0.8:0.7, pan=0.3

loop: 0.0, 16.0
```

## Note Parsing

Notes follow standard music notations:

| Component | Description | Examples |
|-----------|-------------|----------|
| **Base notes** | C, D, E, F, G, A, B | `C`, `D`, `E` |
| **Sharps** | `#` or `S` suffix | `C#4`, `DS5` |
| **Flats** | `b`, `F`, or `B` suffix | `Db3`, `EF4`, `GB2` |
| **Octaves** | Number suffix (C4 = middle C) | `C4`, `A3`, `E5` |

**Frequency calculation:**
- Base frequencies start at C0 = 16.35 Hz
- Each semitone multiplies by 2^(1/12)
- Each octave doubles the frequency

## Details

### Audio Engine

| Feature | Implementation |
|---------|----------------|
| **Backend** | `cpal` for cross-platform audio |
| **Sample rate** | System default (typically 44.1kHz or 48kHz) |
| **Bit depth** | 32-bit float processing |

### Effects Implementation Details

| Effect | Algorithm |
|--------|-----------|
| **Reverb** | Freeverb with 8 comb filters and 4 allpass filters |
| **Delay** | Circular buffer with feedback loop |
| **Distortion** | Cubic waveshaping with tone control lowpass filter |
| **Filters** | Biquad IIR filters with proper coefficient calculation |
