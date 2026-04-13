use std::{
    collections::HashMap,
    net::TcpListener,
    os::fd::{AsRawFd, RawFd},
    ptr,
};

use application::engine_thread::process;
use domain::exchange::Exchange;
use io_uring::{IoUring, opcode, types};

use crate::{
    codec::{decode_command, encode_response},
    session::Session,
};

const OP_ACCEPT: u8 = 0;
const OP_READ: u8 = 1;
const OP_WRITE: u8 = 2;

pub fn run(addr: &str) {
    pin_to_core(0);
    let listener = TcpListener::bind(addr).expect("failed to bind");
    listener
        .set_nonblocking(true)
        .expect("failed to set nonblocking");
    let listener_fd = listener.as_raw_fd();
    unsafe {
        let one = 1 as libc::c_int;
        libc::setsockopt(
            listener_fd,
            libc::IPPROTO_TCP,
            libc::TCP_NODELAY,
            &one as *const _ as *const libc::c_void,
            std::mem::size_of::<libc::c_int>() as libc::socklen_t,
        );
    }

    let mut ring = IoUring::new(256).expect("failed to create io_uring");
    let mut sessions: HashMap<i32, Session> = HashMap::with_capacity(1_000_000);
    let mut exchange = Exchange::new(1_000_000);
    let mut engine_seq = 0u64;
    let mut write_bufs: HashMap<i32, Vec<u8>> = HashMap::with_capacity(1_000_000);

    submit_accept(&mut ring, listener_fd);

    loop {
        ring.submit_and_wait(1).expect("submit_and_wait failed");
        let cqes = ring.completion().collect::<Vec<_>>();

        for cqe in cqes {
            let (fd, op) = decode_token(cqe.user_data());
            let result = cqe.result();

            match op {
                OP_ACCEPT => {
                    submit_accept(&mut ring, listener_fd);
                    if result < 0 {
                        continue;
                    }

                    let client_fd = result;

                    unsafe {
                        let one = 1 as libc::c_int;
                        libc::setsockopt(
                            client_fd,
                            libc::IPPROTO_TCP,
                            libc::TCP_NODELAY,
                            &one as *const _ as *const libc::c_void,
                            std::mem::size_of::<libc::c_int>() as libc::socklen_t,
                        );
                    }

                    sessions.insert(client_fd, Session::new(client_fd));
                    submit_read(&mut ring, &sessions, client_fd);
                }
                OP_READ => {
                    if result <= 0 {
                        sessions.remove(&fd);
                        write_bufs.remove(&fd);
                        unsafe {
                            libc::close(fd);
                        }
                        continue;
                    }

                    let n = result as usize;
                    let session = sessions.get_mut(&fd).unwrap();
                    session.read_pos += n;

                    // Accumulate all responses into a single write buffer
                    let write_buf = write_bufs.entry(fd).or_default();
                    write_buf.clear();

                    while let Some((msg_type, payload)) = session.try_parse_frame() {
                        if let Some(cmd) = decode_command(msg_type, payload) {
                            let response = process(&mut exchange, &mut engine_seq, cmd);
                            encode_response(&response, write_buf);
                        }
                        session.consume_frame();
                    }

                    // Only submit write if there's data; otherwise go straight to next read
                    if !write_buf.is_empty() {
                        submit_write(&mut ring, &write_bufs, fd);
                    } else {
                        write_bufs.remove(&fd);
                        submit_read(&mut ring, &sessions, fd);
                    }
                }
                OP_WRITE => {
                    if result <= 0 {
                        sessions.remove(&fd);
                        write_bufs.remove(&fd);
                        unsafe {
                            libc::close(fd);
                        }
                        continue;
                    }

                    let n = result as usize;
                    let write_buf = write_bufs.get_mut(&fd).unwrap();

                    if n < write_buf.len() {
                        write_buf.drain(..n);
                        submit_write(&mut ring, &write_bufs, fd);
                    } else {
                        write_bufs.remove(&fd);
                        submit_read(&mut ring, &sessions, fd);
                    }
                }
                _ => {}
            }
        }
    }
}

fn pin_to_core(core_id: usize) {
    unsafe {
        let mut set: libc::cpu_set_t = std::mem::zeroed();
        libc::CPU_SET(core_id, &mut set);
        let result = libc::sched_setaffinity(0, size_of::<libc::cpu_set_t>(), &set);

        if result == 0 {
            println!("pinned to core {}", core_id);
        } else {
            eprintln!("failed to pin to core {}", core_id);
        }
    }
}

fn submit_accept(ring: &mut IoUring, listener_fd: RawFd) {
    let accept = opcode::Accept::new(types::Fd(listener_fd), ptr::null_mut(), ptr::null_mut())
        .build()
        .user_data(encode_token(listener_fd, OP_ACCEPT));

    unsafe {
        ring.submission().push(&accept).expect("sq full");
    }
}

fn submit_read(ring: &mut IoUring, sessions: &HashMap<i32, Session>, fd: i32) {
    let session = &sessions[&fd];
    let buf_ptr = unsafe { session.read_buf.as_ptr().add(session.read_pos) as *mut u8 };
    let buf_len = (session.read_buf.len() - session.read_pos) as u32;
    let read = opcode::Read::new(types::Fd(fd), buf_ptr, buf_len)
        .build()
        .user_data(encode_token(fd, OP_READ));

    unsafe {
        ring.submission().push(&read).expect("sq full");
    }
}

fn submit_write(ring: &mut IoUring, write_bufs: &HashMap<i32, Vec<u8>>, fd: i32) {
    let buf = &write_bufs[&fd];
    let write = opcode::Write::new(types::Fd(fd), buf.as_ptr(), buf.len() as u32)
        .build()
        .user_data(encode_token(fd, OP_WRITE));

    unsafe {
        ring.submission().push(&write).expect("sq_full");
    }
}

fn encode_token(fd: i32, op: u8) -> u64 {
    ((fd as u64) << 8) | (op as u64)
}

fn decode_token(token: u64) -> (i32, u8) {
    ((token >> 8) as i32, (token & 0xFF) as u8)
}
