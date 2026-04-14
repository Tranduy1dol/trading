use std::{
    collections::HashMap,
    net::TcpListener,
    os::fd::{AsRawFd, RawFd},
    ptr,
};

use application::command::Command::{AddOrder, CancelOrder, ModifyOrder};
use application::engine_thread::process;
use domain::exchange::Exchange;
use io_uring::{IoUring, opcode, types};

use crate::{
    codec::{decode_command, encode_response},
    journal::Journal,
    protocol::{BboUpdateMsg, MSG_BBO_UPDATE},
    session::Session,
};

const OP_ACCEPT: u8 = 0;
const OP_READ: u8 = 1;
const OP_WRITE: u8 = 2;
const OP_JOURNAL_WRITE: u8 = 3;

pub fn run(addr: &str, journal_path: &str) {
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
    let mut journal_writes: HashMap<u64, Vec<u8>> = HashMap::with_capacity(1_000_000);
    let mut journal_write_id = 0u64;

    println!("replay journal from {}", journal_path);
    let frames = Journal::read_all_frames(journal_path).unwrap();
    let mut replayed = 0;
    for frame in frames {
        let msg_type = frame[4];
        let payload = &frame[5..];

        if let Some(cmd) = decode_command(msg_type, payload) {
            let _ = process(&mut exchange, &mut engine_seq, cmd);
            replayed += 1;
        }
    }
    println!(
        "replayed {} commands. current engine_seq {}",
        replayed, engine_seq
    );

    let journal = Journal::open(journal_path).unwrap();
    let journal_fd = journal.fd;

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
                    let mut client_frames = Vec::new();
                    {
                        let session = sessions.get_mut(&fd).unwrap();
                        session.read_pos += n;
                        while let Some((msg_type, payload)) = session.try_parse_frame() {
                            let frame_len = 5 + payload.len();
                            client_frames.push((msg_type, session.read_buf[0..frame_len].to_vec()));
                            session.consume_frame();
                        }
                    }

                    let mut pending_journal_data = Vec::new();

                    for (msg_type, full_frame) in client_frames {
                        let payload = &full_frame[5..];
                        if let Some(cmd) = decode_command(msg_type, payload) {
                            let asset_id = match &cmd {
                                AddOrder { order, .. } => order.asset_id,
                                CancelOrder { asset_id, .. } => *asset_id,
                                ModifyOrder { asset_id, .. } => *asset_id,
                            };

                            let response = process(&mut exchange, &mut engine_seq, cmd);

                            {
                                let write_buf = write_bufs.entry(fd).or_default();
                                encode_response(&response, write_buf);
                            }

                            let events = exchange.drain_market_data(asset_id);
                            let mut broadcast_buf = Vec::new();

                            for ev in events {
                                if let domain::market_data::MarketDataEvent::LevelUpdated {
                                    price,
                                    side,
                                    total_qty,
                                } = ev
                                {
                                    let bbo = BboUpdateMsg {
                                        engine_seq,
                                        asset_id,
                                        price: price.0,
                                        quantity: total_qty,
                                        side: side as u8,
                                    };

                                    let payload_size = std::mem::size_of::<BboUpdateMsg>();
                                    let len = (1 + payload_size) as u32;

                                    broadcast_buf.extend_from_slice(&len.to_le_bytes());
                                    broadcast_buf.push(MSG_BBO_UPDATE);

                                    let ptr = &bbo as *const _ as *const u8;
                                    unsafe {
                                        broadcast_buf.extend_from_slice(
                                            std::slice::from_raw_parts(ptr, payload_size),
                                        );
                                    }
                                }
                            }

                            if !broadcast_buf.is_empty() {
                                let keys: Vec<i32> = sessions.keys().copied().collect();
                                for active_fd in keys {
                                    {
                                        let client_write_buf =
                                            write_bufs.entry(active_fd).or_default();
                                        client_write_buf.extend_from_slice(&broadcast_buf);
                                    }
                                    if active_fd != fd {
                                        submit_write(&mut ring, &write_bufs, active_fd);
                                    }
                                }
                            }

                            pending_journal_data.extend_from_slice(&full_frame);
                        }
                    }

                    if !pending_journal_data.is_empty() {
                        journal_write_id += 1;
                        let id = journal_write_id;
                        let ptr = pending_journal_data.as_ptr();
                        let len = pending_journal_data.len() as u32;
                        journal_writes.insert(id, pending_journal_data);

                        let write = opcode::Write::new(types::Fd(journal_fd), ptr, len)
                            .offset(0xFFFFFFFFFFFFFFFF)
                            .build()
                            .user_data(encode_token(id as i32, OP_JOURNAL_WRITE));
                        unsafe {
                            ring.submission().push(&write).expect("sq full");
                        }
                    }

                    let needs_write = write_bufs.get(&fd).map(|b| !b.is_empty()).unwrap_or(false);
                    if needs_write {
                        submit_write(&mut ring, &write_bufs, fd);
                    } else {
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
                OP_JOURNAL_WRITE => {
                    let id = fd as u64;
                    journal_writes.remove(&id);
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
