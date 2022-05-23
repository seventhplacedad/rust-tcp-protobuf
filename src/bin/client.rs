use std::io::prelude::*;
use std::net::TcpListener;
use std::net::TcpStream;

use protobuf::Message;
use rust_tcp_protobuf::buster;
use rust_tcp_protobuf::pdu;
use rust_tcp_protobuf::protos::my_messages::*;

fn main() {
    let addr = "192.168.0.117:7878";
    let mut stream = TcpStream::connect(addr).unwrap();
    stream.write(&[97u8, 0, 0, 0, 1, 98u8]).unwrap();
}
