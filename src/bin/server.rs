use core::panic;
use log::{debug, error, info, trace, warn};
use simplelog::*;
use std::fs::File;
use std::net::SocketAddr;
use std::net::TcpListener;
use std::io::prelude::*;
use std::net::TcpStream;
use std::vec;

use rust_tcp_protobuf::libserver::*;

// Err(err) => {
//     match err {
//         pdu::PDUReadErr::GotEOF => {
//             warn!("Client gave an EOF when reading possible PDU. Removing.",)
//         }
//         other_error => error!(
//             "Reading PDU from client gave unexpected error. {:?}",
//             other_error
//         ),
//     }
//     to_remove_indecies.push(i);
// }

fn main() {
    CombinedLogger::init(vec![
        TermLogger::new(
            LevelFilter::Trace,
            Config::default(),
            TerminalMode::Mixed,
            ColorChoice::Auto,
        ),
        WriteLogger::new(
            LevelFilter::Trace,
            Config::default(),
            File::create("myserver.log").unwrap(),
        ),
    ])
    .unwrap();

    let listener = TcpListener::bind("0.0.0.0:7878").unwrap();
    listener.set_nonblocking(true).unwrap();
    let mut clients: Vec<Client> = Vec::new();

    info!("Server is initialized.");
    loop {
        match listener.accept() {
            Ok((stream, addr_info)) => {
                stream.set_nonblocking(true).unwrap();
                info!(
                    "New Client Connected, added to stream list. {:?} ",
                    addr_info
                );

                clients.push(Client::new(stream, addr_info));
            }
            Err(ref error) if error.kind() == std::io::ErrorKind::WouldBlock => (),
            Err(_) => panic!("Unexpected Error"),
        }

        let result: Vec<ServerBlobResult<TryClientToOutblobsErr>> = clients
            .iter_mut()
            .map(client_maybe_pdu_to_outblobs)
            .collect();

        for (i, client_result) in result.into_iter().enumerate().rev() {
            match client_result {
                Err(error) => {
                    warn!(
                        "Going to remove client for this reason: {:?}. Client={:?}",
                        error, clients[i].info
                    );
                    let client = clients.remove(i);
                    client
                        .stream
                        .shutdown(std::net::Shutdown::Both)
                        .expect("Failed to shutdown!");
                }
                Ok(maybe_blobs) => {
                    info!("Would have send these blobs {:?}", maybe_blobs)
                }
            }
        }
    }
}
