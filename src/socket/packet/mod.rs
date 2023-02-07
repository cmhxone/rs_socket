use std::os::fd::RawFd;

use lazy_static::lazy_static;
use log::info;

use crate::socket::get_peer_name;

lazy_static! {
    static ref PACKET_HEADER_LENGTH: usize = dotenv::var("PACKET_HEADER_LENGTH")
        .unwrap()
        .parse::<usize>()
        .unwrap_or(16);

    static ref PACKET_TYPE_HELLO: Vec<u8> = b"++".to_vec();
    static ref PACKET_TYPE_BYE: Vec<u8> = b"--".to_vec();
}

pub fn handle_packet(sockfd: RawFd, buf: Vec<u8>) {
    info!(
        "packet received from peer: {:?}, packet: {:?}",
        get_peer_name(sockfd as RawFd),
        String::from_utf8_lossy(&buf).trim()
    );
}
