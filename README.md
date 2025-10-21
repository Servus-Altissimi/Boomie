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

  Made for Liefde.
```

Real-time music synth and composition engine written in Rust with support for custom file formats, dynamic playback control, and advanced audio effects. Ideal for real time playback.

## Features

### Audio Synthesis
- **Waveform types**: Sine, Square, Triangle, Sawtooth, and Noise
- **Sample-based playback**: Load and play WAV files with pitch adjustment
- **ADSR envelope shaping**: Full Attack, Decay, Sustain, Release control
- **Real-time synthesis**: Low-latency audio output using `cpal`

### Effects Processing
- **Reverb**: Freeverb-based algorithm with room size, damping, wet/dry mix, and width controls
- **Delay**: Configurable delay time, feedback, and wet/dry mix
- **Distortion**: Drive, tone control, and wet/dry mix

### File Formats

#### `.mel`, Melody Files
Define individual tracks with notes, instruments, and effects:
```
name: melody
waveform: sine
tempo: 120
volume: 0.8
attack: 0.01
decay: 0.1
sustain: 0.7
release: 0.2

note: C4, 1.0, 0.8
note: E4, 0.5, 0.9
note: G4, 1.0, 0.7

reverb: 0.6, 0.5, 0.3, 1.0
```

#### `.bmi`, Arrangement Files (Bundled Music Index)
Compose multiple melody tracks into complete songs:
```
name: song
track: bass.mel, 0.0
track: melody.mel, 2.0, volume=0.9
track: drums.mel, 4.0, pitch=1.2
loop: 0.0, 16.0
```

### Compilation & Usage

Add to your `Cargo.toml` Dependecies:

```toml
# or newer when available
cpal = "0.16.0" 
fastrand = "2.3.0"
hound = "3.5.1"
```

## Example

```rust
mod boomie;
use boomie::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut engine = SynthEngine::new()?;
    
    // Load a sample
    engine.load_sample("kick", "samples/kick.wav")?;
    
    // Load melodies
    engine.load_melody("bass", "melodies/bass.mel")?;
    engine.load_melody("lead", "melodies/lead.mel")?;
    
    // Load and play an arrangement
    let arrangement = engine.load_arrangement("songs/main.bmi")?;
    engine.play_arrangement(arrangement)?;
    
    // Enable looping
    engine.set_loop_enabled(true);
    
    // Dynamic control
    engine.set_master_volume(0.8);
    engine.set_track_volume("bass", 1.2);
    
    engine.stop();
    Ok(())
}
```

## API Reference

### Core Functions

- `SynthEngine::new()`: Create a new synthesizer engine
- `load_sample(name, path)`: Load a `.wav` file into the sample cache
- `load_melody(name, path)`: Load a `.mel` file
- `load_arrangement(path)`: Load a `.bmi` file
- `play_arrangement(arrangement)`: Start playback
- `stop()`: Stop playback and clean up

### Playback Control

- `pause()` V `resume()`: Pause and resume playback
- `set_loop_enabled(bool)`: Enable/disable looping
- `crossfade_to(arrangement, duration)`: Smooth transition to a new arrangement
- `get_playback_position()`: Get current playback time
- `get_playback_state()`: Get current state (Playing/Paused/Stopped)

### Dynamic Parameters

- `set_master_volume(volume)`: Set global volume (0.0-2.0)
- `set_master_pitch(pitch)`: Set global pitch multiplier (0.5-2.0)
- `set_track_enabled(name, enabled)`: Enable/disable a track
- `set_track_volume(name, volume)`: Set track volume (0.0-2.0)
- `interpolate_track_volume(name, target, duration)`: Gradual volume change

## File Format Reference

### Melody File (`.mel`)

**Metadata:**
- `name:` - Melody name
- `tempo:` - BPM (default: 120)
- `loop:` - Loop points in seconds, `start, end`

**Instrument:**
- `waveform:` - sine | square | triangle | sawtooth | noise
- `sample:` - Reference to loaded sample name
- `volume:` - Amplitude (0.0-1.0)
- `pitch:` - Pitch multiplier
- `attack:`, `decay:`, `sustain:`, `release:` - ADSR envelope

**Notes:**
- `note: PITCH, DURATION, VELOCITY`
- Pitch: C4, D#5, Gb3, etc.
- Duration: In beats
- Velocity: 0.0-1.0

**Effects:**
- `reverb: room_size, damping, wet, width`
- `delay: time, feedback, wet`
- `distortion: drive, tone, wet`

### Arrangement File (`.bmi`)

**Metadata:**
- `name:` - Arrangement name
- `loop:` - Loop points: `start, end`

**Tracks:**
- `track: melody_file.mel, start_time, [overrides...]`

**Override parameters:**
- `volume=0.8` or `vol=0.8`
- `pitch=1.2`
- `tempo=140`
- `reverb=0.6:0.5:0.3:1.0`
- `delay=0.25:0.4:0.3`
- `distortion=2.0:0.7:0.5` or `dist=...`

## Note Parsing

Notes follow standard music notation:
- Base notes: C, D, E, F, G, A, B
- Sharps: C#, D#, etc. (also accepts 'S')
- Flats: Db, Eb, etc. (also accepts 'F' or 'B')
- Octaves: C4 (middle C), A3, E5, etc.

## Technical Details

- Built on `cpal` for consistent audio output
- Freeverb algorithm for reverb effect
- Linear interpolation for sample playback
- Automatic gain normalization to prevent clipping

