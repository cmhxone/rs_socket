pub mod socket {
    use std::os::fd::RawFd;

    use nix::sys::socket::{getpeername, SockaddrIn};

    ///
    /// 소켓 파일디스크립터의 peer 정보를 반환
    ///
    pub fn get_peer_name(sockfd: RawFd) -> String {
        match getpeername::<SockaddrIn>(sockfd) {
            Ok(addr) => addr.to_string(),
            Err(_) => String::from("Unknown"),
        }
    }
}
