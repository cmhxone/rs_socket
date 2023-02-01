use std::{
    net::TcpListener,
    os::fd::{AsRawFd, RawFd},
    thread,
};

use nix::{
    sys::{
        epoll::{epoll_create, epoll_ctl, epoll_wait, EpollEvent, EpollFlags, EpollOp},
        socket::{accept, getpeername, SockaddrIn},
    },
    unistd::{close, read},
};

use threadpool::ThreadPool;

///
/// 소켓 서버 메인
///
fn main() {
    dotenv::dotenv().unwrap();

    // 풀 사이즈 설정파일에서 바인딩
    let pool_size = dotenv::var("MAX_THREAD_POOL")
        .unwrap()
        .parse::<usize>()
        .unwrap_or(4);
    let pool = ThreadPool::new(pool_size);

    // 커넥션 분산제어용 Epoll 벡터 생성
    let mut epfds = Vec::new();
    for _i in 0..pool.max_count() {
        let epfd = epoll_create().unwrap();
        epfds.push(epfd.clone());
        pool.execute(move || handle_epoll(epfd.clone()));
    }

    // TCP 소켓 리스너 생성
    let ip = dotenv::var("IP_ADDR").unwrap();
    let port = dotenv::var("PORT").unwrap();
    let listener = TcpListener::bind(format!("{}:{}", ip, port)).unwrap();
    listener.set_nonblocking(true).unwrap();

    // TCP 소켓 핸들링 Epoll 파일 디스크립터 생성(UNIX)
    let epfd = epoll_create().unwrap();
    let sockfd = listener.as_raw_fd();
    let mut event = EpollEvent::new(EpollFlags::EPOLLET | EpollFlags::EPOLLIN, sockfd as u64);

    // TCP 소켓 리스너 Epoll 관심목록 등록(UNIX)
    epoll_ctl(epfd, EpollOp::EpollCtlAdd, sockfd, &mut event).unwrap();

    // TCP 소켓 리스너 Epoll 루프 동작(UNIX)
    let mut events = [EpollEvent::empty(); 1];
    loop {
        match epoll_wait(epfd, &mut events, 1_000) {
            Ok(size) => {
                for i in 0..size {
                    match (events[i].data(), events[i].events()) {
                        (fd, _ev) if fd as RawFd == sockfd => {
                            // 랜덤한 Epoll 파일디스크립터에 추가
                            let idx = rand::random::<usize>() % pool.max_count();
                            let epfd = epfds.clone().get(idx).unwrap().clone();
                            let stream_fd = accept(sockfd).unwrap();
                            println!(
                                "connect from peer {}",
                                getpeername::<SockaddrIn>(stream_fd as RawFd)
                                    .unwrap()
                                    .to_string()
                            );

                            // TCP 스트림 핸들러 연동 스레드 호출
                            thread::spawn(move || handle_stream(epfd, stream_fd));
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
fn handle_stream(_epfd: RawFd, _sockfd: RawFd) -> () {
    let mut event = EpollEvent::new(
        EpollFlags::EPOLLIN | EpollFlags::EPOLLET | EpollFlags::EPOLLRDHUP,
        _sockfd as u64,
    );
    epoll_ctl(_epfd, EpollOp::EpollCtlAdd, _sockfd.as_raw_fd(), &mut event).unwrap();
}

///
/// Epoll 루프 핸들러
///
fn handle_epoll(_epfd: RawFd) -> () {
    let mut events = [EpollEvent::empty(); 2];
    loop {
        match epoll_wait(_epfd, &mut events, 100) {
            Ok(size) => {
                for _i in 0..size {
                    match (events[_i].data(), events[_i].events()) {
                        (_fd, _ev) if _ev == EpollFlags::EPOLLIN => {
                            // 패킷 입력 처리
                            let mut buf = [0; 128];
                            match read(_fd as RawFd, &mut buf) {
                                Ok(size) if size > 0 => {
                                    println!(
                                        "packet received from peer: {:?}, packet: {}",
                                        getpeername::<SockaddrIn>(_fd as RawFd)
                                            .unwrap()
                                            .to_string(),
                                        String::from_utf8_lossy(&buf)
                                    );
                                }
                                Ok(_) => {}
                                Err(error) => {
                                    eprintln!("read error: {:?}", error);
                                }
                            }
                        }
                        (_fd, _ev) if _ev == EpollFlags::EPOLLRDHUP | EpollFlags::EPOLLIN => {
                            // 접속 해제 처리
                            println!(
                                "disconnected from peer {:?}",
                                getpeername::<SockaddrIn>(_fd as RawFd).unwrap().to_string()
                            );
                            let mut event = EpollEvent::new(
                                EpollFlags::EPOLLET | EpollFlags::EPOLLIN | EpollFlags::EPOLLRDHUP,
                                _fd,
                            );
                            epoll_ctl(_epfd, EpollOp::EpollCtlDel, _fd as RawFd, &mut event)
                                .unwrap();
                            close(_fd as RawFd).unwrap();
                        }
                        (_fd, _ev) => {
                            println!("{:?}", _ev);
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
