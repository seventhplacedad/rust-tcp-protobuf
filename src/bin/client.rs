use std::io::prelude::*;
use std::net::TcpListener;
use std::net::TcpStream;

fn main() {
    let addr = "127.0.0.1:7878";
    let mut stream = TcpStream::connect(addr).unwrap();
    stream.write(& [97u8, 0, 0, 0, 1, 98u8]).unwrap();
}
