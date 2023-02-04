use std::os::fd::{AsRawFd, RawFd};

use nix::{
    sys::{
        epoll::{epoll_ctl, EpollEvent, EpollFlags, EpollOp},
        socket::{getpeername, SockaddrIn},
    },
    unistd::close,
};

///
/// 소켓 파일디스크립터의 peer 정보를 반환
///
pub fn get_peer_name(sockfd: RawFd) -> String {
    match getpeername::<SockaddrIn>(sockfd) {
        Ok(addr) => addr.to_string(),
        Err(_) => String::from("Unknown"),
    }
}

///
/// Epoll 소켓 파일디스크립터 관심 대상 등록
///
pub fn add_epoll_list(epfd: RawFd, sockfd: RawFd) -> Result<(), nix::errno::Errno> {
    let mut event = EpollEvent::new(epoll_monitor_event(), sockfd as u64);
    epoll_ctl(epfd, EpollOp::EpollCtlAdd, sockfd.as_raw_fd(), &mut event)
}

///
/// Epoll 소켓 파일디스크립터 관심 대상 해제
///
pub fn remove_epoll_list(epfd: RawFd, sockfd: RawFd) -> Result<(), nix::errno::Errno> {
    match epoll_ctl(
        epfd,
        EpollOp::EpollCtlDel,
        sockfd as RawFd,
        &mut EpollEvent::empty(),
    ) {
        Ok(_) => close(sockfd as RawFd),
        Err(error) => Err(error),
    }
}

///
/// Epoll 모니터링 대상 이벤트 반환
///
fn epoll_monitor_event() -> EpollFlags {
    EpollFlags::EPOLLET | EpollFlags::EPOLLIN | EpollFlags::EPOLLRDHUP
}
