use std::{
    io::{self, Read},
    net::{Shutdown, TcpListener, TcpStream},
    sync::{Arc, Mutex},
};

use threadpool::ThreadPool;

fn main() {
    let listener = TcpListener::bind("0.0.0.0:7878").unwrap();
    let pool = ThreadPool::new(16);

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
    stream.lock().unwrap().set_nonblocking(true).unwrap();
    println!(
        "peer connected: {:?}",
        stream.lock().unwrap().peer_addr().unwrap()
    );

    let mut buf = [0; 128];
    loop {
        match stream.lock().unwrap().read(&mut buf) {
            Ok(size) if size > 0 => {
                println!("buffer({}): {}", size, String::from_utf8_lossy(&buf));
            }
            Ok(_) => {
                println!(
                    "peer disconnected: {:?}",
                    stream.lock().unwrap().peer_addr().unwrap()
                );
                stream.lock().unwrap().shutdown(Shutdown::Both).unwrap();
                return;
            }
            Err(ref _e) if _e.kind() == io::ErrorKind::WouldBlock => {
                // Unimplemented...
            }
            Err(err) => {
                println!("error: {:?}", err);
            }
        }
    }
}
