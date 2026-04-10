use std::os::fd::RawFd;

use crate::protocol::HEADER_SIZE;

pub const READ_BUF_SIZE: usize = 4096;

pub struct Session {
    pub fd: RawFd,
    pub read_buf: Box<[u8; READ_BUF_SIZE]>,
    pub read_pos: usize,
}

impl Session {
    pub fn new(fd: RawFd) -> Self {
        Self {
            fd,
            read_buf: Box::new([0u8; READ_BUF_SIZE]),
            read_pos: 0,
        }
    }

    pub fn try_parse_frame(&self) -> Option<(u8, &[u8])> {
        if self.read_pos < HEADER_SIZE {
            return None;
        }

        let len = u32::from_le_bytes([
            self.read_buf[0],
            self.read_buf[1],
            self.read_buf[2],
            self.read_buf[3],
        ]) as usize;

        let total_frame_size = 4 + len;

        if self.read_pos < total_frame_size {
            return None;
        }

        let msg_type = self.read_buf[4];
        let payload = &self.read_buf[5..total_frame_size];

        Some((msg_type, payload))
    }

    pub fn consume_frame(&mut self) {
        let len = u32::from_le_bytes([
            self.read_buf[0],
            self.read_buf[1],
            self.read_buf[2],
            self.read_buf[3],
        ]) as usize;

        let total = 4 + len;
        let remaining = self.read_pos - total;

        if remaining > 0 {
            self.read_buf.copy_within(total..self.read_pos, 0);
        }
        self.read_pos = remaining;
    }
}
