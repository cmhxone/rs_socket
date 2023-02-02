use std::{
    net::TcpListener,
    os::fd::{AsRawFd, RawFd},
    thread,
};

use lazy_static::lazy_static;
use nix::{
    sys::{
        epoll::{epoll_create, epoll_ctl, epoll_wait, EpollEvent, EpollFlags, EpollOp},
        socket::{accept, setsockopt, sockopt},
    },
    unistd::{close, read},
};

use threadpool::ThreadPool;

use rs_socket::socket::get_peer_name;

lazy_static! {
    /// 스레드 풀 최대 스레드 수
    static ref POOL_SIZE: usize = dotenv::var("MAX_THREAD_POOL")
        .unwrap()
        .parse::<usize>()
        .unwrap_or(4);

    /// 소켓 바인딩 IP
    static ref IP_ADDR: String = dotenv::var("IP_ADDR").unwrap();
    /// 소켓 바인딩 포트
    static ref PORT: String = dotenv::var("PORT").unwrap();

    /// 패킷 버퍼 길이
    static ref PACKET_LENGTH: usize = dotenv::var("PACKET_LENGTH")
        .unwrap()
        .parse::<usize>()
        .unwrap_or(1_024);

    /// 소켓 바인딩 Epoll 타임아웃
    static ref EPOLL_BINDER_TIMEOUT: isize = dotenv::var("EPOLL_BINDER_TIMEOUT").unwrap().parse::<isize>().unwrap_or(1_000);
    /// 소켓 핸들링 Epoll 타임아웃
    static ref EPOLL_HANDLER_TIMEOUT: isize = dotenv::var("EPOLL_HANDLER_TIMEOUT").unwrap().parse::<isize>().unwrap_or(100);

    /// 소켓 바인딩 Epoll 파일 디스크립터 이벤트 처리 수
    static ref EPOLL_BINDER_EVENT_COUNT: usize = dotenv::var("EPOLL_BINDER_EVENT_COUNT")
        .unwrap()
        .parse::<usize>()
        .unwrap_or(1_024);
    /// 소켓 핸들링 Epoll 파일 디스크립터 이벤트 처리 수
    static ref EPOLL_HANDLER_EVENT_COUNT: usize = dotenv::var("EPOLL_HANDLER_EVENT_COUNT")
        .unwrap()
        .parse::<usize>()
        .unwrap_or(1_024);

    /// 소켓 Idle 타임아웃(초)
    static ref SOCKET_IDLE_TIMEOUT_SEC: u32 = dotenv::var("SOCKET_IDLE_TIMEOUT_SEC").unwrap().parse::<u32>().unwrap_or(30);
}

///
/// 소켓 서버 메인
///
fn main() {
    dotenv::dotenv().unwrap();

    let pool = ThreadPool::new(*POOL_SIZE);

    // 커넥션 분산제어용 Epoll 벡터 생성
    let mut epfds = Vec::new();
    for _i in 0..pool.max_count() {
        let epfd = epoll_create().unwrap();
        epfds.push(epfd.clone());
        pool.execute(move || handle_epoll(epfd.clone()));
    }

    // TCP 소켓 리스너 생성
    let listener = TcpListener::bind(format!("{}:{}", *IP_ADDR, *PORT)).unwrap();
    listener.set_nonblocking(true).unwrap(); // 논블로킹 소켓 설정 활성화

    // TCP 소켓 핸들링 Epoll 파일 디스크립터 생성(UNIX)
    let epfd = epoll_create().unwrap();
    let sockfd = listener.as_raw_fd();
    let mut event = EpollEvent::new(EpollFlags::EPOLLET | EpollFlags::EPOLLIN, sockfd as u64);

    // TCP 소켓 리스너 Epoll 관심목록 등록(UNIX)
    epoll_ctl(epfd, EpollOp::EpollCtlAdd, sockfd, &mut event).unwrap();

    // TCP 소켓 리스너 Epoll 루프 동작(UNIX)
    let mut events = vec![EpollEvent::empty(); *EPOLL_BINDER_EVENT_COUNT];
    loop {
        match epoll_wait(epfd, &mut events, *EPOLL_BINDER_TIMEOUT) {
            Ok(size) => {
                for i in 0..size {
                    match (events[i].data(), events[i].events()) {
                        (fd, _ev) if fd as RawFd == sockfd => {
                            // 랜덤한 Epoll 파일디스크립터에 추가
                            let idx = rand::random::<usize>() % pool.max_count();
                            let epfd = epfds.clone().get(idx).unwrap().clone();
                            let connfd = accept(sockfd).unwrap();

                            // 네이글(Nagle) 알고리즘 소켓 설정 활성화
                            setsockopt(connfd, sockopt::TcpNoDelay, &true).unwrap();

                            // TCP Keep-alive 소켓 설정 활성화
                            let keep_alive = true;
                            setsockopt(connfd, sockopt::KeepAlive, &keep_alive).unwrap();
                            let keep_idle = *SOCKET_IDLE_TIMEOUT_SEC;
                            setsockopt(connfd, sockopt::TcpKeepIdle, &keep_idle).unwrap();
                            println!("connect from peer {:?}", get_peer_name(connfd as RawFd));

                            // TCP 스트림 핸들러 연동 스레드 호출
                            thread::spawn(move || bind_socket(epfd, connfd));
                        }
                        _ => {}
                    }
                }
            }
            Err(error) => {
                eprintln!("main epoll wait error: {:?}", error);
            }
        }
    }
}

///
/// TCP 스트림 Epoll 파일 디스크립터 관심 목록 등록 핸들러
///
fn bind_socket(epfd: RawFd, sockfd: RawFd) -> () {
    let mut event = EpollEvent::new(
        EpollFlags::EPOLLIN | EpollFlags::EPOLLET | EpollFlags::EPOLLRDHUP,
        sockfd as u64,
    );
    epoll_ctl(epfd, EpollOp::EpollCtlAdd, sockfd.as_raw_fd(), &mut event).unwrap();
}

///
/// Epoll 루프 핸들러
///
fn handle_epoll(epfd: RawFd) -> () {
    let mut events = vec![EpollEvent::empty(); *EPOLL_HANDLER_EVENT_COUNT];
    loop {
        match epoll_wait(epfd, &mut events, *EPOLL_HANDLER_TIMEOUT) {
            Ok(size) => {
                for i in 0..size {
                    match (events[i].data(), events[i].events()) {
                        (fd, ev) if ev == EpollFlags::EPOLLIN => {
                            // 패킷 입력 처리
                            let mut buf = vec![0; *PACKET_LENGTH];
                            match read(fd as RawFd, &mut buf) {
                                Ok(size) if size > 0 => {
                                    println!(
                                        "packet received from peer: {:?}, packet: {}",
                                        get_peer_name(fd as RawFd),
                                        String::from_utf8_lossy(&buf)
                                    );
                                }
                                Ok(_) => {}
                                Err(error) => {
                                    eprintln!(
                                        "read error from peer: {:?}, {:?}",
                                        get_peer_name(fd as RawFd),
                                        error
                                    );
                                }
                            }
                        }
                        (fd, ev) if ev == EpollFlags::EPOLLRDHUP | EpollFlags::EPOLLIN => {
                            // 접속 해제 처리
                            println!("disconnected from peer {:?}", get_peer_name(fd as RawFd));
                            let mut event = EpollEvent::new(
                                EpollFlags::EPOLLET | EpollFlags::EPOLLIN | EpollFlags::EPOLLRDHUP,
                                fd,
                            );
                            epoll_ctl(epfd, EpollOp::EpollCtlDel, fd as RawFd, &mut event).unwrap();
                            close(fd as RawFd).unwrap();
                        }
                        (fd, ev) => {
                            println!("epoll_handler(): {:?}", ev);
                            let mut event = EpollEvent::new(
                                EpollFlags::EPOLLET | EpollFlags::EPOLLIN | EpollFlags::EPOLLRDHUP,
                                fd,
                            );
                            epoll_ctl(epfd, EpollOp::EpollCtlDel, fd as RawFd, &mut event).unwrap();
                            close(fd as RawFd).unwrap();
                        }
                    }
                }
            }
            Err(error) => {
                eprintln!("handle_epoll wait error: {:?}", error);
            }
        }
    }
}
