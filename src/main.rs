use std::{
    io::{Read, Write},
    net::{Shutdown, TcpListener, TcpStream},
    os::fd::AsRawFd,
    sync::{Arc, Mutex},
};

use nix::sys::epoll::{epoll_create, epoll_ctl, epoll_wait, EpollEvent, EpollFlags, EpollOp};
use threadpool::ThreadPool;

/// 패킷 사이즈
const PACKET_SIZE: usize = 128;
/// 패킷 헤더 사이즈
const HEADER_SIZE: usize = 4;

/// 패킷 헤더유형(등록)
const PACKET_TYPE_REGISTER: &[u8] = "....".as_bytes();
/// 패킷 헤더유형(Keep-Alive)
const PACKET_TYPE_KEEPALIVE: &[u8] = "..!!".as_bytes();

/// epoll 이벤트 크기
const EPOLL_EVENT_COUNT: usize = 1_024;
/// epoll 타임아웃
const EPOLL_TIMEOUT: isize = 1_000;

fn main() {
    dotenv::dotenv().unwrap();

    // TCP 소켓, 스레드풀 생성
    let listener = TcpListener::bind("0.0.0.0:7878").unwrap();
    let pool = ThreadPool::new(
        dotenv::var("MAX_PEER")
            .unwrap_or(String::from("16"))
            .parse::<usize>()
            .unwrap(),
    );

    // 스레드 풀 수(최대 클라이언트 접속 수)
    println!("Maximum peer count: {}", pool.max_count());

    // TCP accept 발생 시, 스레드풀로 전달
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

    // Client 피어 정보
    let peer_addr = stream.lock().unwrap().peer_addr().unwrap();
    println!("peer connected: {:?}", peer_addr);

    // Unix Epoll 구현
    let sock_fd = stream.lock().unwrap().as_raw_fd();
    let epoll_fd = epoll_create().unwrap();
    let mut epoll_event = EpollEvent::new(EpollFlags::EPOLLIN, sock_fd as u64);

    epoll_ctl(epoll_fd, EpollOp::EpollCtlAdd, sock_fd, &mut epoll_event).unwrap();

    let mut connected = true;
    while connected {
        match epoll_wait(
            epoll_fd,
            &mut [EpollEvent::new(EpollFlags::EPOLLIN, 0); EPOLL_EVENT_COUNT],
            EPOLL_TIMEOUT,
        ) {
            Ok(size) if size > 0 => {
                // epoll 파일디스크립터 사이즈 변경 확인
                let mut buf = [0; PACKET_SIZE];
                match stream.lock().unwrap().read(&mut buf) {
                    Ok(size) if size > 0 => {
                        // 패킷 수신
                        handle_buffer(&buf);
                    }
                    Ok(_) => {
                        // 패킷 0Byte 수신(연결종료)
                        println!("peer disconnected: {:?}", peer_addr);
                        connected = false;
                    }
                    Err(err) => {
                        // 소켓 Read 오류
                        eprintln!("socket read error: {:?}", err);
                        connected = false;
                    }
                };
            }
            Ok(_) => {
                // epoll 타임아웃 발생 시, Keep-Alive 패킷 송신
                stream.lock().unwrap().write(PACKET_TYPE_KEEPALIVE).unwrap();
                stream.lock().unwrap().flush().unwrap();
            }
            Err(err) => {
                // epoll 오류
                eprintln!("epoll error: {:?}", err);
            }
        }
    }

    // TCP Stream 종료
    stream.lock().unwrap().shutdown(Shutdown::Both).unwrap();
}

///
/// TCP 버퍼 핸들러
///
fn handle_buffer(buf: &[u8]) {
    let buf_head = &buf[0..HEADER_SIZE]; // 패킷 헤더
    let _buf_body = &buf[HEADER_SIZE..]; // 패킷 본문

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
