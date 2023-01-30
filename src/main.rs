use std::{
    io::Read,
    net::{Shutdown, TcpListener, TcpStream},
    sync::{Arc, Mutex},
};

use threadpool::ThreadPool;

const PACKET_SIZE: usize = 128;
const HEADER_SIZE: usize = 4;
const PACKET_TYPE_REGISTER: &[u8] = "aabb".as_bytes();

fn main() {
    let listener = TcpListener::bind("0.0.0.0:7878").unwrap();
    let pool = ThreadPool::new(32);

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                pool.execute(|| handle_stream(stream));
            }
            Err(err) => {
                println!("error: {:?}", err);
            }
        }
    }
}

///
/// TCP 스트림 커넥션 핸들러
///
fn handle_stream(stream: TcpStream) {
    let stream = Arc::new(Mutex::new(stream));
    stream.lock().unwrap().set_nonblocking(false).unwrap();

    let peer_addr = stream.lock().unwrap().peer_addr().unwrap();
    println!("peer connected: {:?}", peer_addr);

    let mut connected = true;
    while connected {
        let mut buf = [0; PACKET_SIZE];
        match stream.lock().unwrap().read(&mut buf) {
            Ok(size) if size > 0 => {
                handle_buffer(&buf);
            }
            Ok(_) => {
                println!("peer disconnected: {:?}", peer_addr);
                connected = false;
            }
            Err(err) => {
                println!("error: {:?}", err);
                connected = false;
            }
        };
    }

    stream.lock().unwrap().shutdown(Shutdown::Both).unwrap();
}

// TCP 버퍼 핸들러
fn handle_buffer(buf: &[u8]) {
    let buf_head = &buf[0..HEADER_SIZE];
    let _buf_body = &buf[HEADER_SIZE..];

    match buf_head {
        PACKET_TYPE_REGISTER => {
            println!("Matched here");
            // println!("buf_body: {:?}", buf_body);
        }
        _ => {
            // println!("buf_head: {:?}", buf_head);
            // println!("buf_body: {:?}", buf_body);
        }
    }
}
