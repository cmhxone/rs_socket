use std::{
    io::{ErrorKind, Read, Write},
    net::{Shutdown, TcpListener, TcpStream},
    os::fd::AsRawFd,
    sync::{Arc, Mutex},
};

use nix::sys::epoll::{epoll_create, epoll_ctl, epoll_wait, EpollEvent, EpollFlags, EpollOp};
use threadpool::ThreadPool;

const PACKET_SIZE: usize = 128;
const HEADER_SIZE: usize = 4;
const PACKET_TYPE_REGISTER: &[u8] = "aabb".as_bytes();
const PACKET_TYPE_KEEPALIVE: &[u8] = "keal".as_bytes();

fn main() {
    let listener = TcpListener::bind("0.0.0.0:7878").unwrap();
    let pool = ThreadPool::new(32);

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                pool.execute(|| handle_stream(stream));
            }
            Err(err) => {
                eprintln!("socket incoming error: {:?}", err);
            }
        }
    }
}

///
/// TCP 스트림 커넥션 핸들러
///
fn handle_stream(stream: TcpStream) {
    let stream = Arc::new(Mutex::new(stream));
    stream.lock().unwrap().set_nonblocking(true).unwrap();

    let peer_addr = stream.lock().unwrap().peer_addr().unwrap();
    println!("peer connected: {:?}", peer_addr);

    let sock_fd = stream.lock().unwrap().as_raw_fd();
    let epoll_fd = epoll_create().unwrap();
    let mut epoll_event = EpollEvent::new(EpollFlags::EPOLLIN, sock_fd as u64);

    epoll_ctl(epoll_fd, EpollOp::EpollCtlAdd, sock_fd, &mut epoll_event).unwrap();

    let mut connected = true;
    while connected {
        match epoll_wait(
            epoll_fd,
            &mut [EpollEvent::new(EpollFlags::EPOLLIN, 0); 1024],
            1_000,
        ) {
            Ok(size) if size > 0 => {
                let mut buf = [0; PACKET_SIZE];
                match stream.lock().unwrap().read(&mut buf) {
                    Ok(size) if size > 0 => {
                        handle_buffer(&buf);
                    }
                    Ok(_) => {
                        println!("peer disconnected: {:?}", peer_addr);
                        connected = false;
                    }
                    Err(err) if err.kind() == ErrorKind::WouldBlock => {}
                    Err(err) => {
                        eprintln!("socket read error: {:?}", err);
                        connected = false;
                    }
                };
            }
            Ok(_) => {
                stream.lock().unwrap().write(PACKET_TYPE_KEEPALIVE).unwrap();
                stream.lock().unwrap().flush().unwrap();
            }
            Err(err) => {
                eprintln!("epoll error: {:?}", err);
            }
        }
    }

    stream.lock().unwrap().shutdown(Shutdown::Both).unwrap();
}

///
/// TCP 버퍼 핸들러
///
fn handle_buffer(buf: &[u8]) {
    let buf_head = &buf[0..HEADER_SIZE];
    let _buf_body = &buf[HEADER_SIZE..];

    match buf_head {
        PACKET_TYPE_REGISTER => {
            println!("Matched here");
            // println!("buf_body: {:?}", buf_body);
        }
        _ => {
            println!("Some packet received");
            // println!("buf_head: {:?}", buf_head);
            // println!("buf_body: {:?}", buf_body);
        }
    }
}
