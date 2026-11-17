//! Write an iTunes freeform publisher tag without displacing media data.
//! FFmpeg's mdta mode loses attached pictures on some supported versions.
use crate::*;
use std::io::{Read, Seek, SeekFrom};
fn atom(kind: &[u8; 4], data: &[u8]) -> Result<Vec<u8>> {
    let len = u32::try_from(data.len().checked_add(8).context("Atom size overflow")?)?;
    Ok([len.to_be_bytes().as_slice(), kind, data].concat())
}
fn child(file: &mut fs::File, start: u64, end: u64, kind: &[u8; 4]) -> Result<(u64, u64)> {
    let mut offset = start;
    while offset + 8 <= end {
        file.seek(SeekFrom::Start(offset))?;
        let mut header = [0; 8];
        file.read_exact(&mut header)?;
        let size = u64::from(u32::from_be_bytes(header[..4].try_into()?));
        ensure!(
            size >= 8 && size <= end - offset,
            "Invalid MP4 metadata atom"
        );
        if &header[4..] == kind {
            return Ok((offset, size));
        }
        offset += size;
    }
    bail!("Missing MP4 metadata container")
}
pub(crate) fn publisher(path: &Path, publisher: &str) -> Result<()> {
    if publisher.is_empty() {
        return Ok(());
    }
    ensure!(publisher.len() <= 65536, "Publisher tag too large");
    let mut file = fs::OpenOptions::new().read(true).write(true).open(path)?;
    let length = file.metadata()?.len();
    let mut offset = 0;
    while offset + 8 <= length {
        file.seek(SeekFrom::Start(offset))?;
        let mut header = [0; 8];
        file.read_exact(&mut header)?;
        let mut size = u64::from(u32::from_be_bytes(header[..4].try_into()?));
        let mut header_size = 8;
        if size == 1 {
            let mut extended = [0; 8];
            file.read_exact(&mut extended)?;
            size = u64::from_be_bytes(extended);
            header_size = 16;
        }
        ensure!(
            size >= header_size && size <= length - offset,
            "Invalid MP4 container size"
        );
        if &header[4..] == b"moov" {
            ensure!(
                offset + size == length && header_size == 8,
                "Publisher requires trailing MP4 metadata"
            );
            let field = atom(
                b"----",
                &[
                    atom(b"mean", &[&[0; 4][..], b"com.apple.iTunes"].concat())?,
                    atom(b"name", &[&[0; 4][..], b"publisher"].concat())?,
                    atom(
                        b"data",
                        &[&[0, 0, 0, 1, 0, 0, 0, 0][..], publisher.as_bytes()].concat(),
                    )?,
                ]
                .concat(),
            )?;
            let udta = child(&mut file, offset + 8, offset + size, b"udta")?;
            let meta = child(&mut file, udta.0 + 8, udta.0 + udta.1, b"meta")?;
            ensure!(meta.1 >= 12, "Invalid MP4 metadata");
            let ilst = child(&mut file, meta.0 + 12, meta.0 + meta.1, b"ilst")?;
            let growth = field.len() as u64;
            let containers = [(offset, size), udta, meta, ilst];
            for &(_, len) in &containers {
                u32::try_from(len.checked_add(growth).context("Atom size overflow")?)?;
            }
            let insertion = ilst.0 + ilst.1;
            let mut cursor = length;
            let mut buffer = [0u8; 65536];
            while cursor > insertion {
                let count = (cursor - insertion).min(buffer.len() as u64) as usize;
                cursor -= count as u64;
                file.seek(SeekFrom::Start(cursor))?;
                file.read_exact(&mut buffer[..count])?;
                file.seek(SeekFrom::Start(cursor + growth))?;
                file.write_all(&buffer[..count])?;
            }
            file.seek(SeekFrom::Start(insertion))?;
            file.write_all(&field)?;
            for (pos, len) in containers {
                file.seek(SeekFrom::Start(pos))?;
                file.write_all(&u32::try_from(len + growth)?.to_be_bytes())?;
            }
            return Ok(());
        }
        offset += size;
    }
    bail!("Missing MP4 movie metadata")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn large_trailing_movie_is_rewritten_without_moving_media() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("large.m4b");
        let ilst = atom(b"ilst", &[]).unwrap();
        let meta = atom(b"meta", &[&[0; 4][..], &ilst].concat()).unwrap();
        let udta = atom(b"udta", &meta).unwrap();
        let free = atom(b"free", &vec![7; 17 * 1024 * 1024]).unwrap();
        let moov = atom(b"moov", &[udta, free].concat()).unwrap();
        let media = atom(b"mdat", b"media bytes").unwrap();
        fs::write(&path, [&media[..], &moov].concat()).unwrap();
        publisher(&path, "Publisher").unwrap();
        let result = fs::read(&path).unwrap();
        assert_eq!(&result[..media.len()], media);
        assert!(result.windows(9).any(|w| w == b"Publisher"));
        assert_eq!(
            result.len() as u64,
            media.len() as u64
                + u32::from_be_bytes(result[media.len()..media.len() + 4].try_into().unwrap())
                    as u64
        );
    }
}
