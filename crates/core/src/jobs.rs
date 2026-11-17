//! Shared cancellation, deadlines and bounded subprocess capture.
use crate::*;
use std::{io::Read, process::Output, time::Instant};
#[derive(Clone, Debug)]
pub struct Job {
    pub cancel: Cancel,
    pub deadline: Option<Instant>,
}
impl Default for Job {
    fn default() -> Self {
        Self {
            cancel: Arc::new(AtomicBool::new(false)),
            deadline: None,
        }
    }
}
impl Job {
    pub fn new(cancel: Cancel, timeout: Duration) -> Self {
        Self {
            cancel,
            deadline: Some(Instant::now() + timeout),
        }
    }
    pub fn check(&self) -> Result<()> {
        ensure!(
            !self.cancel.load(AtomicOrdering::Relaxed),
            "Operation cancelled"
        );
        ensure!(
            self.deadline.is_none_or(|d| Instant::now() < d),
            "Operation deadline exceeded"
        );
        Ok(())
    }
}
pub(crate) fn capture(command: &mut Command, job: &Job, limit: usize) -> Result<Output> {
    let job = Job {
        cancel: job.cancel.clone(),
        deadline: Some(
            job.deadline
                .unwrap_or_else(|| Instant::now() + Duration::from_secs(120))
                .min(Instant::now() + Duration::from_secs(120)),
        ),
    };
    job.check()?;
    let mut child = command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("Start bounded media process")?;
    fn drain(
        mut pipe: impl Read + Send + 'static,
        limit: usize,
    ) -> std::thread::JoinHandle<std::io::Result<(Vec<u8>, bool)>> {
        std::thread::spawn(move || {
            let mut kept = Vec::new();
            let mut buf = [0; 8192];
            let mut overflow = false;
            loop {
                let n = pipe.read(&mut buf)?;
                if n == 0 {
                    break;
                }
                let keep = n.min(limit.saturating_sub(kept.len()));
                kept.extend_from_slice(&buf[..keep]);
                overflow |= keep < n;
            }
            Ok((kept, overflow))
        })
    }
    let out = drain(child.stdout.take().unwrap(), limit);
    let err = drain(child.stderr.take().unwrap(), 65536);
    let result = loop {
        if let Err(e) = job.check() {
            let _ = child.kill();
            let _ = child.wait();
            break Err(e);
        }
        match child.try_wait() {
            Ok(Some(status)) => break Ok(status),
            Ok(None) => std::thread::sleep(Duration::from_millis(20)),
            Err(e) => {
                let _ = child.kill();
                let _ = child.wait();
                break Err(e.into());
            }
        }
    };
    let (stdout, overflow) = out
        .join()
        .map_err(|_| anyhow::anyhow!("Output reader failed"))??;
    let (stderr, _) = err
        .join()
        .map_err(|_| anyhow::anyhow!("Error reader failed"))??;
    let status = result?;
    ensure!(!overflow, "Process output exceeds limit");
    Ok(Output {
        status,
        stdout,
        stderr,
    })
}
/// Includes both decoded stereo PCM staging and the final file, plus reserve.
pub fn required_export_bytes(p: &Project) -> u64 {
    if p.can_remux() {
        return p.tracks.iter().map(|t| &t.path).chain(p.cover.iter()).fold(
            64 * 1024 * 1024,
            |sum, path| {
                sum.saturating_add(
                    fs::metadata(path)
                        .map(|m| m.len())
                        .unwrap_or(u64::MAX)
                        .saturating_mul(2),
                )
            },
        );
    }
    p.duration_ms()
        .saturating_mul(192 + 192)
        .saturating_add(64 * 1024 * 1024)
}
pub fn preflight(p: &Project, directory: &Path) -> Result<()> {
    let available = available_space(directory)?;
    ensure!(
        available >= required_export_bytes(p),
        "Insufficient disk space: require {} bytes, available {} bytes",
        required_export_bytes(p),
        available
    );
    Ok(())
}
#[cfg(unix)]
pub fn available_space(path: &Path) -> Result<u64> {
    use std::{ffi::CString, os::unix::ffi::OsStrExt};
    let path = CString::new(path.as_os_str().as_bytes())?;
    let mut stat = std::mem::MaybeUninit::<libc::statvfs>::uninit();
    ensure!(
        unsafe { libc::statvfs(path.as_ptr(), stat.as_mut_ptr()) } == 0,
        "Cannot inspect free disk space: {}",
        std::io::Error::last_os_error()
    );
    let stat = unsafe { stat.assume_init() };
    Ok((u128::from(stat.f_bavail) * u128::from(stat.f_frsize)).min(u128::from(u64::MAX)) as u64)
}
#[cfg(windows)]
pub fn available_space(path: &Path) -> Result<u64> {
    use std::os::windows::ffi::OsStrExt;
    #[link(name = "kernel32")]
    extern "system" {
        fn GetDiskFreeSpaceExW(
            path: *const u16,
            available: *mut u64,
            total: *mut u64,
            free: *mut u64,
        ) -> i32;
    }
    let path: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    let mut available = 0;
    ensure!(
        unsafe {
            GetDiskFreeSpaceExW(
                path.as_ptr(),
                &mut available,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
        } != 0,
        "Cannot inspect free disk space"
    );
    Ok(available)
}
/// Stream one validated member into a generated file, with no unbounded buffer.
pub(crate) fn stream_file(
    command: &mut Command,
    job: &Job,
    destination: &Path,
    maximum: u64,
    password: Option<&str>,
) -> Result<u64> {
    let job = Job {
        cancel: job.cancel.clone(),
        deadline: Some(
            job.deadline
                .unwrap_or_else(|| Instant::now() + Duration::from_secs(120))
                .min(Instant::now() + Duration::from_secs(120)),
        ),
    };
    job.check()?;
    let mut child = command
        .stdin(if password.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()?;
    let writer = password.map(|password| {
        let mut input = child.stdin.take().expect("Piped archiver input");
        let password = password.to_owned();
        std::thread::spawn(move || writeln!(input, "{password}"))
    });
    let mut pipe = child.stdout.take().unwrap();
    let path = destination.to_path_buf();
    let reader = std::thread::spawn(move || -> Result<u64> {
        let mut output = fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(path)?;
        let mut total = 0u64;
        let mut buf = [0; 65536];
        loop {
            let n = pipe.read(&mut buf)?;
            if n == 0 {
                break;
            }
            total += n as u64;
            ensure!(total <= maximum, "Archive member exceeds declared size");
            output.write_all(&buf[..n])?;
        }
        output.sync_all()?;
        Ok(total)
    });
    let mut reader = Some(reader);
    let mut read_result = None;
    let status = loop {
        if let Err(e) = job.check() {
            let _ = child.kill();
            let _ = child.wait();
            break Err(e);
        }
        // A reader failure must stop a child that continues writing or sleeping.
        if reader.as_ref().is_some_and(|r| r.is_finished()) {
            read_result = Some(
                reader
                    .take()
                    .unwrap()
                    .join()
                    .map_err(|_| anyhow::anyhow!("Archive reader failed"))
                    .and_then(|r| r),
            );
            if read_result.as_ref().is_some_and(|r| r.is_err()) {
                let _ = child.kill();
                let _ = child.wait();
                break Err(anyhow::anyhow!("Archive stream failed"));
            }
        }
        match child.try_wait() {
            Ok(Some(status)) => break Ok(status),
            Ok(None) => std::thread::sleep(Duration::from_millis(20)),
            Err(e) => {
                let _ = child.kill();
                let _ = child.wait();
                break Err(e.into());
            }
        }
    };
    if let Some(writer) = writer {
        let _ = writer.join();
    }
    let total = match read_result {
        Some(r) => r?,
        None => reader
            .unwrap()
            .join()
            .map_err(|_| anyhow::anyhow!("Archive reader failed"))??,
    };
    ensure!(
        status?.success(),
        "Archive extraction failed (wrong password, corrupt or unsupported archive)"
    );
    Ok(total)
}

pub(crate) fn copy_file(source: &Path, destination: &Path, job: &Job) -> Result<()> {
    use std::io::Read;
    let mut input = fs::File::open(source)?;
    let mut output = fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(destination)?;
    let mut buffer = [0; 65536];
    loop {
        job.check()?;
        let n = input.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        output.write_all(&buffer[..n])?;
    }
    output.sync_all()?;
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(unix)]
    #[test]
    fn deadline_reaps_and_drains_large_stderr() {
        let mut c = Command::new("sh");
        c.args(["-c", "exec sleep 30"]);
        let start = Instant::now();
        assert!(capture(
            &mut c,
            &Job::new(Arc::new(AtomicBool::new(false)), Duration::from_millis(50)),
            1024
        )
        .is_err());
        assert!(start.elapsed() < Duration::from_secs(2));
        let mut c = Command::new("sh");
        c.args(["-c", "head -c 100000 /dev/zero >&2"]);
        assert!(capture(&mut c, &Job::default(), 1024)
            .unwrap()
            .status
            .success());
    }
}
