//! Deliberately restricted ZIP reader: stored/deflate, no encryption, ZIP64 or multi-volume.
use anyhow::{ensure, Context, Result};
use std::{
    fs::File,
    io::{Read, Seek, SeekFrom},
};
fn u16le(b: &[u8], n: usize) -> u16 {
    u16::from_le_bytes([b[n], b[n + 1]])
}
fn u32le(b: &[u8], n: usize) -> u32 {
    u32::from_le_bytes(b[n..n + 4].try_into().unwrap())
}
struct Record {
    name: String,
    size: u32,
    compressed: u32,
    crc: u32,
    offset: u32,
    method: u16,
    mode: Option<u32>,
}
pub struct ZipArchive {
    file: File,
    records: Vec<Record>,
    directory: u64,
}
pub struct Entry {
    name: String,
    size: u64,
    mode: Option<u32>,
    reader: Box<dyn Read>,
    crc: crc32fast::Hasher,
    expected: u32,
    read: u64,
}
impl ZipArchive {
    pub fn new(mut file: File) -> Result<Self> {
        let len = file.metadata()?.len();
        let tail_len = len.min(65557);
        file.seek(SeekFrom::End(-(tail_len as i64)))?;
        let mut tail = vec![0; tail_len as usize];
        file.read_exact(&mut tail)?;
        let end = (0..tail.len().saturating_sub(21))
            .rev()
            .find(|&i| {
                tail[i..].starts_with(b"PK\x05\x06")
                    && i + 22 + u16le(&tail, i + 20) as usize == tail.len()
            })
            .context("ZIP end record missing")?;
        let b = &tail[end..];
        ensure!(
            u16le(b, 4) == 0 && u16le(b, 6) == 0 && u16le(b, 8) == u16le(b, 10),
            "Multi-volume ZIP unsupported"
        );
        let count = u16le(b, 10);
        ensure!(
            count <= 4096 && u32le(b, 12) != u32::MAX && u32le(b, 16) != u32::MAX,
            "ZIP64 or excessive entries unsupported"
        );
        let directory = u32le(b, 16) as u64;
        ensure!(
            directory + u32le(b, 12) as u64 <= len - tail_len + end as u64,
            "Invalid ZIP directory bounds"
        );
        let directory_end = directory + u32le(b, 12) as u64;
        file.seek(SeekFrom::Start(directory))?;
        let mut records = Vec::new();
        for _ in 0..count {
            ensure!(
                file.stream_position()? + 46 <= directory_end,
                "Truncated ZIP directory record"
            );
            let mut h = [0; 46];
            file.read_exact(&mut h)?;
            ensure!(&h[..4] == b"PK\x01\x02", "Invalid ZIP directory");
            ensure!(
                u16le(&h, 8) & !0x080e == 0 && [0, 8].contains(&u16le(&h, 10)),
                "Encrypted/unsupported ZIP compression; use stored or deflate"
            );
            ensure!(
                u16le(&h, 34) == 0
                    && u32le(&h, 20) != u32::MAX
                    && u32le(&h, 24) != u32::MAX
                    && u32le(&h, 42) != u32::MAX,
                "ZIP64/multi-volume unsupported"
            );
            ensure!(u16le(&h, 28) <= 4096, "ZIP filename exceeds 4096 bytes");
            let mut name = vec![0; u16le(&h, 28) as usize];
            file.read_exact(&mut name)?;
            let name = String::from_utf8(name).context("ZIP filenames must be UTF-8")?;
            ensure!(
                file.stream_position()? + u16le(&h, 30) as u64 + u16le(&h, 32) as u64
                    <= directory_end,
                "ZIP directory fields exceed bounds"
            );
            file.seek(SeekFrom::Current(
                u16le(&h, 30) as i64 + u16le(&h, 32) as i64,
            ))?;
            records.push(Record {
                name,
                size: u32le(&h, 24),
                compressed: u32le(&h, 20),
                crc: u32le(&h, 16),
                offset: u32le(&h, 42),
                method: u16le(&h, 10),
                mode: if h[5] == 3 {
                    Some(u32le(&h, 38) >> 16)
                } else {
                    None
                },
            });
        }
        ensure!(
            file.stream_position()? == directory_end,
            "ZIP directory size mismatch"
        );
        Ok(Self {
            file,
            records,
            directory,
        })
    }
    pub fn len(&self) -> usize {
        self.records.len()
    }
    pub fn by_index(&mut self, i: usize) -> Result<Entry> {
        let r = self
            .records
            .get(i)
            .context("ZIP entry index out of bounds")?;
        ensure!(
            r.offset as u64 + 30 <= self.directory,
            "ZIP local header overlaps directory"
        );
        self.file.seek(SeekFrom::Start(r.offset as u64))?;
        let mut h = [0; 30];
        self.file.read_exact(&mut h)?;
        ensure!(
            &h[..4] == b"PK\x03\x04" && u16le(&h, 6) & !0x080e == 0 && u16le(&h, 8) == r.method,
            "Invalid ZIP local header"
        );
        ensure!(
            u16le(&h, 26) as usize == r.name.len(),
            "ZIP local filename length mismatch"
        );
        let mut name = vec![0; u16le(&h, 26) as usize];
        self.file.read_exact(&mut name)?;
        ensure!(
            name == r.name.as_bytes(),
            "ZIP local/central filename mismatch"
        );
        let start = r.offset as u64 + 30 + name.len() as u64 + u16le(&h, 28) as u64;
        ensure!(
            start + r.compressed as u64 <= self.directory,
            "Truncated ZIP entry"
        );
        ensure!(
            r.method != 0 || r.compressed == r.size,
            "Stored ZIP size mismatch"
        );
        // Independent handle: validation does not allocate decompressed data.
        let mut file = self.file.try_clone()?;
        file.seek(SeekFrom::Start(start))?;
        let input = file.take(r.compressed as u64);
        let reader: Box<dyn Read> = if r.method == 8 {
            Box::new(flate2::read::DeflateDecoder::new(input))
        } else {
            Box::new(input)
        };
        Ok(Entry {
            name: r.name.clone(),
            size: r.size as u64,
            mode: r.mode,
            reader,
            crc: crc32fast::Hasher::new(),
            expected: r.crc,
            read: 0,
        })
    }
}
impl Entry {
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn size(&self) -> u64 {
        self.size
    }
    pub fn unix_mode(&self) -> Option<u32> {
        self.mode
    }
    pub fn is_dir(&self) -> bool {
        self.name.ends_with('/')
    }
}
impl Read for Entry {
    fn read(&mut self, b: &mut [u8]) -> std::io::Result<usize> {
        let n = self.reader.read(b)?;
        self.read += n as u64;
        self.crc.update(&b[..n]);
        if self.read > self.size
            || (n == 0 && (self.read != self.size || self.crc.clone().finalize() != self.expected))
        {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "ZIP size/CRC mismatch",
            ));
        }
        Ok(n)
    }
}
