use std::{
    fs::OpenOptions,
    io::Read,
    os::fd::{IntoRawFd, RawFd},
    path::Path,
};

use crate::protocol::HEADER_SIZE;

pub struct Journal {
    pub fd: RawFd,
}

impl Journal {
    pub fn open<P: AsRef<Path>>(path: P) -> std::io::Result<Self> {
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .read(true)
            .open(path)?;

        Ok(Self {
            fd: file.into_raw_fd(),
        })
    }

    pub fn read_all_frames<P: AsRef<Path>>(path: P) -> std::io::Result<Vec<Vec<u8>>> {
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(path)?;
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer)?;

        let mut frames = Vec::new();
        let mut pos = 0;

        while pos + HEADER_SIZE <= buffer.len() {
            let len = u32::from_le_bytes([
                buffer[pos],
                buffer[pos + 1],
                buffer[pos + 2],
                buffer[pos + 3],
            ]) as usize;

            let total_size = 4 + len;
            if pos + total_size > buffer.len() {
                break;
            }

            frames.push(buffer[pos..pos + total_size].to_vec());
            pos += total_size;
        }

        Ok(frames)
    }
}
