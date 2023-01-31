use std::{
    net::{Shutdown, TcpListener, TcpStream},
    os::fd::AsRawFd,
    sync::{Arc, Mutex},
};

use nix::{
    sys::epoll::{epoll_create, epoll_ctl, epoll_wait, EpollEvent, EpollFlags, EpollOp},
    unistd::{read, write},
};
use threadpool::ThreadPool;

/// 패킷 사이즈
const PACKET_SIZE: usize = 128;
/// 패킷 헤더 사이즈
const HEADER_SIZE: usize = 4;

/// 패킷 헤더유형(등록)
const PACKET_TYPE_REGISTER: &[u8] = "....".as_bytes();

/// epoll 이벤트 크기
const EPOLL_EVENT_COUNT: usize = 1_024;

fn main() {
    dotenv::dotenv().unwrap();

    // TCP 소켓, 스레드풀 생성
    let listener = TcpListener::bind("0.0.0.0:7878").unwrap();
    let pool = ThreadPool::new(
        dotenv::var("MAX_THREAD_POOL")
            .unwrap_or(String::from("16"))
            .parse::<usize>()
            .unwrap(),
    );

    // 스레드 풀 수(최대 클라이언트 접속 수)
    println!("Maximum peer count: {}", pool.max_count());
    // Epoll 아이들 타임아웃
    println!(
        "Epoll timeout: {}",
        dotenv::var("EPOLL_IDLE_TIMEOUT")
            .unwrap()
            .parse::<isize>()
            .unwrap_or(10_000)
    );

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

    let epoll_timeout = dotenv::var("EPOLL_IDLE_TIMEOUT")
        .unwrap()
        .parse::<isize>()
        .unwrap_or(10_000);

    let mut connected = true;
    while connected {
        let mut events = [EpollEvent::new(EpollFlags::empty(), 0); EPOLL_EVENT_COUNT];
        match epoll_wait(epoll_fd, &mut events, epoll_timeout) {
            Ok(size) if size > 0 => {
                // epoll 파일디스크립터 사이즈 변경 확인
                let mut buf = [0; PACKET_SIZE];
                match read(sock_fd, &mut buf) {
                    Ok(size) if size > 0 => {
                        // 패킷 수신
                        handle_buffer(sock_fd, &buf);
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
            Ok(_) => {}
            Err(err) => {
                // epoll 오류
                eprintln!("epoll error: {:?}", err);
            }
        }
    }

    // TCP Stream 종료
    stream.lock().unwrap().shutdown(Shutdown::Both).unwrap();
    epoll_ctl(epoll_fd, EpollOp::EpollCtlDel, sock_fd, &mut epoll_event).unwrap();
}

///
/// TCP 버퍼 핸들러
///
fn handle_buffer(sock_fd: i32, buf: &[u8]) {
    let buf_head = &buf[0..HEADER_SIZE]; // 패킷 헤더
    let _buf_body = &buf[HEADER_SIZE..]; // 패킷 본문

    match buf_head {
        PACKET_TYPE_REGISTER => {
            println!("registering {}", String::from_utf8_lossy(_buf_body).trim());
            write(
                sock_fd,
                format!("Hello {}", String::from_utf8_lossy(_buf_body).trim()).as_bytes(),
            )
            .unwrap();
        }
        _ => {
            println!("undefined packet received, {:?}", buf_head);
        }
    }
}
