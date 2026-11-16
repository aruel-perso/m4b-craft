//! Bounded PCM preview. The worker owns the device and every decoder process.
use m4b_core::DesktopFfmpeg;
use rodio::{OutputStream, Sink, Source};
use std::{
    io::{BufReader, Read},
    path::PathBuf,
    process::{Child, Command, Stdio},
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc, Arc,
    },
    thread,
    time::Duration,
};
const RATE: u32 = 48_000;
const CHANNELS: u16 = 2;
const SAMPLES_PER_MS: u64 = 96;

pub enum CommandMessage {
    Play(PathBuf, u64),
    Pause(bool),
    Stop,
    Volume(f32),
    Exit,
}
#[derive(Clone, Default)]
pub struct State {
    pub position_ms: u64,
    pub active: bool,
    pub paused: bool,
    pub message: String,
}
pub struct Playback {
    tx: mpsc::Sender<CommandMessage>,
    pub rx: mpsc::Receiver<State>,
    worker: Option<thread::JoinHandle<()>>,
    pending: Arc<AtomicU64>,
    active: Arc<AtomicBool>,
}
impl Playback {
    pub fn new() -> Self {
        let (tx, commands) = mpsc::channel();
        let (events, rx) = mpsc::sync_channel(1);
        let pending = Arc::new(AtomicU64::new(0));
        let active = Arc::new(AtomicBool::new(false));
        let worker_pending = pending.clone();
        let worker_active = active.clone();
        let worker = thread::spawn(move || run(commands, events, worker_pending, worker_active));
        Self {
            tx,
            rx,
            worker: Some(worker),
            pending,
            active,
        }
    }
    pub fn send(&self, command: CommandMessage) {
        self.pending.fetch_add(1, Ordering::AcqRel);
        if self.tx.send(command).is_err() {
            self.pending.fetch_sub(1, Ordering::AcqRel);
        }
    }
    /// Includes queued play/stop commands and decoder shutdown, unlike UI snapshots.
    pub fn is_busy(&self) -> bool {
        self.pending.load(Ordering::Acquire) > 0 || self.active.load(Ordering::Acquire)
    }
}
impl Drop for Playback {
    fn drop(&mut self) {
        self.send(CommandMessage::Exit);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

/// Global chapter positions use half-open track intervals; the book end maps to
/// the end of the last nonempty track. Zero-length tracks are skipped.
pub fn locate(durations: impl IntoIterator<Item = u64>, mut ms: u64) -> Option<(usize, u64)> {
    let mut last = None;
    for (i, duration) in durations.into_iter().enumerate() {
        if duration == 0 {
            continue;
        }
        if ms < duration {
            return Some((i, ms));
        }
        ms -= duration;
        last = Some((i, duration));
    }
    last
}
pub fn clock(ms: u64) -> String {
    let s = ms / 1000;
    format!("{}:{:02}:{:02}", s / 3600, s / 60 % 60, s % 60)
}
struct Pcm {
    rx: mpsc::Receiver<Vec<i16>>,
    chunk: std::vec::IntoIter<i16>,
    consumed: Arc<AtomicU64>,
}
impl Iterator for Pcm {
    type Item = i16;
    fn next(&mut self) -> Option<i16> {
        let sample = loop {
            if let Some(s) = self.chunk.next() {
                break s;
            }
            self.chunk = self.rx.recv().ok()?.into_iter();
        };
        self.consumed.fetch_add(1, Ordering::Relaxed);
        Some(sample)
    }
}
impl Source for Pcm {
    fn current_frame_len(&self) -> Option<usize> {
        None
    }
    fn channels(&self) -> u16 {
        CHANNELS
    }
    fn sample_rate(&self) -> u32 {
        RATE
    }
    fn total_duration(&self) -> Option<Duration> {
        None
    }
}
struct Decoder {
    child: Child,
    reader: Option<thread::JoinHandle<Result<(), String>>>,
    stderr: Option<thread::JoinHandle<String>>,
    consumed: Arc<AtomicU64>,
    offset: u64,
    sink: Sink,
}
impl Decoder {
    fn stop(mut self) {
        // Kill first to release a reader blocked on the pipe; dropping the sink
        // releases a producer blocked on the bounded PCM queue.
        let _ = self.child.kill();
        self.sink.stop();
        drop(self.sink);
        let _ = self.child.wait();
        if let Some(r) = self.reader.take() {
            let _ = r.join();
        }
        if let Some(r) = self.stderr.take() {
            let _ = r.join();
        }
    }
}
fn pump(mut input: impl Read, tx: mpsc::SyncSender<Vec<i16>>) -> Result<(), String> {
    let mut bytes = [0u8; 8192];
    let mut carry = None;
    loop {
        let n = input.read(&mut bytes).map_err(|e| format!("PCM: {e}"))?;
        if n == 0 {
            return if carry.is_some() {
                Err("Truncated PCM sample".into())
            } else {
                Ok(())
            };
        }
        let mut samples = Vec::with_capacity(n / 2 + 1);
        for &byte in &bytes[..n] {
            if let Some(low) = carry.take() {
                samples.push(i16::from_le_bytes([low, byte]));
            } else {
                carry = Some(byte);
            }
        }
        if tx.send(samples).is_err() {
            return Ok(());
        }
    }
}
fn decoder_command(path: PathBuf, offset: u64) -> Command {
    let mut command = Command::new(DesktopFfmpeg::default().ffmpeg);
    command
        .args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-nostdin",
            "-threads",
            "1",
            "-ss",
        ])
        .arg(format!("{}.{:03}", offset / 1000, offset % 1000))
        .arg("-i")
        .arg(path)
        .args([
            "-map",
            "0:a:0",
            "-vn",
            "-f",
            "s16le",
            "-acodec",
            "pcm_s16le",
            "-ar",
            "48000",
            "-ac",
            "2",
            "pipe:1",
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000); // CREATE_NO_WINDOW
    }
    command
}
fn start(
    path: PathBuf,
    offset: u64,
    handle: &rodio::OutputStreamHandle,
    volume: f32,
) -> Result<Decoder, String> {
    let sink = Sink::try_new(handle).map_err(|e| format!("Audio: {e}"))?;
    start_into_sink(path, offset, sink, volume)
}
fn start_into_sink(path: PathBuf, offset: u64, sink: Sink, volume: f32) -> Result<Decoder, String> {
    let mut child = decoder_command(path, offset)
        .spawn()
        .map_err(|e| format!("FFmpeg: {e}"))?;
    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();
    let stderr = thread::spawn(move || {
        let mut tail = std::collections::VecDeque::with_capacity(8192);
        let mut reader = BufReader::new(stderr);
        let mut buffer = [0; 1024];
        while let Ok(n) = reader.read(&mut buffer) {
            if n == 0 {
                break;
            }
            for byte in &buffer[..n] {
                if tail.len() == 8192 {
                    tail.pop_front();
                }
                tail.push_back(*byte);
            }
        }
        String::from_utf8_lossy(&tail.into_iter().collect::<Vec<_>>()).into_owned()
    });
    // At most ~170 ms queued, plus the current chunk and OS pipe buffers.
    let (tx, rx) = mpsc::sync_channel(4);
    let reader = thread::spawn(move || pump(stdout, tx));
    let consumed = Arc::new(AtomicU64::new(0));
    sink.set_volume(volume);
    sink.append(Pcm {
        rx,
        chunk: Vec::new().into_iter(),
        consumed: consumed.clone(),
    });
    Ok(Decoder {
        child,
        reader: Some(reader),
        stderr: Some(stderr),
        consumed,
        offset,
        sink,
    })
}
fn run(
    commands: mpsc::Receiver<CommandMessage>,
    events: mpsc::SyncSender<State>,
    pending: Arc<AtomicU64>,
    active: Arc<AtomicBool>,
) {
    let mut output = None;
    let mut decoder: Option<Decoder> = None;
    let mut state = State::default();
    let mut volume = 1.;
    loop {
        let command = commands.recv_timeout(Duration::from_millis(50));
        let received = command.is_ok();
        match command {
            Ok(CommandMessage::Exit) | Err(mpsc::RecvTimeoutError::Disconnected) => break,
            Ok(CommandMessage::Stop) => {
                if let Some(d) = decoder.take() {
                    d.stop();
                }
                state = State::default();
            }
            Ok(CommandMessage::Play(path, offset)) => {
                if let Some(d) = decoder.take() {
                    d.stop();
                }
                state = State {
                    position_ms: offset,
                    ..State::default()
                };
                if output.is_none() {
                    match OutputStream::try_default() {
                        Ok(device) => output = Some(device),
                        Err(e) => state.message = format!("Audio device / Périphérique audio: {e}"),
                    }
                }
                if let Some((_, handle)) = &output {
                    match start(path, offset, handle, volume) {
                        Ok(d) => {
                            decoder = Some(d);
                            state.active = true;
                        }
                        Err(e) => state.message = e,
                    }
                }
            }
            Ok(CommandMessage::Pause(paused)) => {
                if let Some(d) = &decoder {
                    if paused {
                        d.sink.pause();
                    } else {
                        d.sink.play();
                    }
                    state.paused = paused;
                }
            }
            Ok(CommandMessage::Volume(v)) => {
                volume = v.clamp(0., 1.);
                if let Some(d) = &decoder {
                    d.sink.set_volume(volume);
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
        }
        if let Some(d) = decoder.as_mut() {
            state.position_ms = d.offset + d.consumed.load(Ordering::Relaxed) / SAMPLES_PER_MS;
            let result = d.child.try_wait();
            let failure = match &result {
                Ok(Some(s)) => !s.success(),
                Err(_) => true,
                _ => false,
            };
            if failure || (matches!(result, Ok(Some(_))) && d.sink.empty()) {
                let mut d = decoder.take().unwrap();
                let reader = d.reader.take().unwrap();
                let errors = d.stderr.take().unwrap();
                let count = d.consumed.load(Ordering::Relaxed);
                d.stop();
                let read_result = reader.join();
                let stderr = errors.join().unwrap_or_default();
                state.message = match result {
                    Err(e) => format!("FFmpeg: {e}"),
                    Ok(Some(status)) if !status.success() => format!("FFmpeg {status}: {stderr}"),
                    _ => match read_result {
                        Ok(Ok(())) if !stderr.trim().is_empty() => format!("FFmpeg: {stderr}"),
                        Ok(Ok(())) if count > 0 => "End of track / Fin de piste".into(),
                        Ok(Ok(())) => "No audio decoded / Aucun audio décodé".into(),
                        Ok(Err(e)) => e,
                        Err(_) => "PCM reader failed / Échec du lecteur PCM".into(),
                    },
                };
                state.active = false;
                state.paused = false;
            }
        }
        active.store(state.active, Ordering::Release);
        if received {
            pending.fetch_sub(1, Ordering::AcqRel);
        }
        let _ = events.try_send(state.clone());
    }
    if let Some(d) = decoder {
        d.stop();
    }
    active.store(false, Ordering::Release);
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn update_guard_includes_queued_commands_and_decoder_shutdown() {
        let (tx, commands) = mpsc::channel();
        let (_events, rx) = mpsc::sync_channel(1);
        let playback = Playback {
            tx,
            rx,
            worker: None,
            pending: Arc::new(AtomicU64::new(0)),
            active: Arc::new(AtomicBool::new(false)),
        };
        playback.send(CommandMessage::Stop);
        // A default/inactive UI state must not hide a command still in flight.
        assert!(playback.is_busy());
        assert!(crate::updater::guard(playback.is_busy(), false, false).is_err());
        assert!(matches!(commands.recv().unwrap(), CommandMessage::Stop));
        playback.active.store(true, Ordering::Release);
        playback.pending.fetch_sub(1, Ordering::AcqRel);
        assert!(playback.is_busy());
        playback.active.store(false, Ordering::Release);
        assert!(!playback.is_busy());
    }
    #[test]
    fn chapter_mapping() {
        assert_eq!(locate([0, 1000, 2000], 1000), Some((2, 0)));
        assert_eq!(locate([1000, 2000], 2999), Some((1, 1999)));
        assert_eq!(locate([1000, 2000], 9000), Some((1, 2000)));
        assert_eq!(locate([0, 0], 0), None);
        assert_eq!(clock(7_261_999), "2:01:01");
    }
    #[test]
    fn pcm_stream_and_consumption() {
        let (tx, rx) = mpsc::sync_channel(4);
        pump(&[0, 128, 255, 127, 0, 0][..], tx).unwrap();
        let consumed = Arc::new(AtomicU64::new(0));
        let pcm = Pcm {
            rx,
            chunk: vec![].into_iter(),
            consumed: consumed.clone(),
        };
        assert_eq!(pcm.collect::<Vec<_>>(), vec![i16::MIN, i16::MAX, 0]);
        assert_eq!(consumed.load(Ordering::Relaxed), 3);
        let (tx, _rx) = mpsc::sync_channel(4);
        assert!(pump(&[1][..], tx).is_err());
    }
    #[test]
    fn fragmented_pcm_and_receiver_disconnect() {
        struct Bytes(std::io::Cursor<Vec<u8>>);
        impl Read for Bytes {
            fn read(&mut self, out: &mut [u8]) -> std::io::Result<usize> {
                self.0.read(&mut out[..1])
            }
        }
        let (tx, rx) = mpsc::sync_channel(4);
        pump(Bytes(std::io::Cursor::new(vec![0, 128, 255, 127])), tx).unwrap();
        assert_eq!(
            rx.into_iter().flatten().collect::<Vec<_>>(),
            vec![i16::MIN, i16::MAX]
        );
        let (tx, rx) = mpsc::sync_channel(1);
        let reader = thread::spawn(move || pump(std::io::repeat(0), tx));
        drop(rx);
        assert!(reader.join().unwrap().is_ok());
    }
    #[test]
    fn rodio_render_pause_resume_volume_seek_and_stop_without_device() {
        // Exercise the same decoder/sink as production, rendering into memory.
        // No physical audio hardware or external player is needed.
        let path = std::env::temp_dir().join(format!("m4b-render-{}.wav", std::process::id()));
        struct Cleanup(PathBuf);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                let _ = std::fs::remove_file(&self.0);
            }
        }
        let _cleanup = Cleanup(path.clone());
        let samples = RATE * 3;
        let bytes = samples * 2;
        let mut wav = Vec::new();
        wav.extend_from_slice(b"RIFF");
        wav.extend_from_slice(&(bytes + 36).to_le_bytes());
        wav.extend_from_slice(b"WAVEfmt ");
        wav.extend_from_slice(&16u32.to_le_bytes());
        wav.extend_from_slice(&1u16.to_le_bytes());
        wav.extend_from_slice(&1u16.to_le_bytes());
        wav.extend_from_slice(&RATE.to_le_bytes());
        wav.extend_from_slice(&(RATE * 2).to_le_bytes());
        wav.extend_from_slice(&2u16.to_le_bytes());
        wav.extend_from_slice(&16u16.to_le_bytes());
        wav.extend_from_slice(b"data");
        wav.extend_from_slice(&bytes.to_le_bytes());
        for i in 0..samples {
            let value = if i % 100 < 50 { 8000i16 } else { -8000i16 };
            wav.extend_from_slice(&value.to_le_bytes());
        }
        std::fs::write(&path, wav).unwrap();
        let (sink, mut rendered) = Sink::new_idle();
        let decoder = start_into_sink(path, 500, sink, 0.5).unwrap();
        let consumed = decoder.consumed.clone();
        let mut nonzero = false;
        for _ in 0..9600 {
            nonzero |= rendered.next().unwrap().abs() > 0.01;
        }
        assert!(
            nonzero,
            "decoder must render actual non-silent samples through rodio"
        );
        assert!(consumed.load(Ordering::Relaxed) > 0);
        assert_eq!(decoder.offset, 500);
        decoder.sink.pause();
        for _ in 0..9600 {
            rendered.next();
        }
        let paused = consumed.load(Ordering::Relaxed);
        for _ in 0..9600 {
            assert_eq!(rendered.next(), Some(0.0));
        }
        assert_eq!(
            consumed.load(Ordering::Relaxed),
            paused,
            "pause must not advance decoded sample position"
        );
        decoder.sink.play();
        for _ in 0..9600 {
            rendered.next();
        }
        assert!(consumed.load(Ordering::Relaxed) > paused);
        decoder.sink.set_volume(0.0);
        for _ in 0..9600 {
            rendered.next();
        }
        for _ in 0..9600 {
            assert_eq!(rendered.next(), Some(0.0));
        }
        drop(rendered);
        decoder.stop(); // kill, drop the bounded queue, and reap decoder threads
    }
    #[test]
    fn ffmpeg_pipe_without_audio_device() {
        // Optional integration test: uses an existing decoder, never downloads one.
        let executable = DesktopFfmpeg::default().ffmpeg;
        if Command::new(&executable)
            .arg("-version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_err()
        {
            eprintln!("Skipping decoder integration test: FFmpeg unavailable");
            return;
        }
        let path = std::env::temp_dir().join(format!("m4b-preview-{}.wav", std::process::id()));
        // One second of mono, 48 kHz, signed 16-bit silence.
        let mut wav = Vec::new();
        wav.extend_from_slice(b"RIFF");
        wav.extend_from_slice(&96036u32.to_le_bytes());
        wav.extend_from_slice(b"WAVEfmt ");
        wav.extend_from_slice(&16u32.to_le_bytes());
        wav.extend_from_slice(&1u16.to_le_bytes());
        wav.extend_from_slice(&1u16.to_le_bytes());
        wav.extend_from_slice(&48000u32.to_le_bytes());
        wav.extend_from_slice(&96000u32.to_le_bytes());
        wav.extend_from_slice(&2u16.to_le_bytes());
        wav.extend_from_slice(&16u16.to_le_bytes());
        wav.extend_from_slice(b"data");
        wav.extend_from_slice(&96000u32.to_le_bytes());
        wav.resize(96044, 0);
        std::fs::write(&path, wav).unwrap();
        let mut child = decoder_command(path.clone(), 500).spawn().unwrap();
        let stdout = child.stdout.take().unwrap();
        let (tx, rx) = mpsc::sync_channel(4);
        let reader = thread::spawn(move || pump(stdout, tx));
        let samples: usize = rx.into_iter().map(|chunk| chunk.len()).sum();
        assert!(reader.join().unwrap().is_ok());
        let result = child.wait_with_output().unwrap();
        let _ = std::fs::remove_file(path);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(samples, 48000); // half a second, stereo
    }
}
