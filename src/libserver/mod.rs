use protobuf::Message;

use crate::pdu;
use crate::protos::my_messages::*;
use std::io::prelude::*;
use std::net::TcpStream;

pub type ServerBlobResult<T> = Result<Option<Vec<OutgoingBlob>>, T>;

#[derive(Debug)]
pub enum BlobDestination {
    Broadcast,
    Multicast(u64),
    Unicast(UnicastAddress),
}

#[derive(Debug)]
pub struct OutgoingBlob {
    pub blob: Vec<u8>,
    pub dest: BlobDestination,
}

#[derive(Debug)]
pub struct ClientInfo {
    pub self_assigned_addr: Option<UnicastAddress>,
    pub socket_address: std::net::SocketAddr,
    pub current_roles: tinyset::Set64<u64>,
}
pub struct Client {
    pub stream: TcpStream,
    pub info: ClientInfo,
}

impl Client {
    pub fn new(stream: TcpStream, socket_address: std::net::SocketAddr) -> Client {
        Client {
            stream,
            info: ClientInfo {
                socket_address,
                self_assigned_addr: None,
                current_roles: tinyset::Set64::new(),
            },
        }
    }
}

pub fn client_maybe_pdu_to_outblobs(client: &mut Client) -> ServerBlobResult<TryClientToOutblobsErr> {
    use pdu::maybe_get_pdu;
    use pdu::PDUReadOk::*;
    use TryClientToOutblobsErr::*;
    match maybe_get_pdu(&mut client.stream).map_err(|e| ReadErr(e))? {
        ReadPDU(blob) => {
            let maybe_outblobs = use_blob(&mut client.info, blob);
            let optional_outblobs = maybe_outblobs.map_err(|e| InvalidDataErr(e))?;
            Ok(optional_outblobs)
        }
        NoPDU => Ok(None),
    }
}

#[derive(Debug)]
pub enum ServerUnpackBlobError {
    BadProtoBlob(protobuf::ProtobufError),
    InvalidMagic,
    MissingFields,
    FailedOutgoingProtoPacking(protobuf::ProtobufError),
}

#[derive(Debug)]
pub enum TryClientToOutblobsErr {
    InvalidDataErr(ServerUnpackBlobError),
    ReadErr(pdu::PDUReadErr),
}

const TOP_LEVEL_MAGIC: u32 = 21093159;
const MANAGEMENT_MAGIC: u32 = 4258764624;


fn use_management_message(
    sender_info: &mut ClientInfo,
    msg: &ManagementMessage,
) -> ServerBlobResult<ServerUnpackBlobError>{
    use ServerUnpackBlobError::*;
    if !msg.has_magic() || msg.get_magic() != MANAGEMENT_MAGIC {
        Err(InvalidMagic)
    } else if msg.has_assign_address() {
        let addr = msg.get_assign_address();
        let owned_addr = addr.clone();
        sender_info.self_assigned_addr = Some(owned_addr);

        let mut state_msg = OtherClientStateMessage::new();
        state_msg.set_addr(addr.clone());
        state_msg.set_state(ClientState::JOINED);
        let mut management_msg: ManagementMessage = ManagementMessage::new();
        management_msg.set_info_other_client_state(state_msg);
        let mut outgoing_notification: TopLevelMessage = TopLevelMessage::new();
        outgoing_notification.set_management(management_msg);

        Ok(Some(vec![OutgoingBlob {
            blob: outgoing_notification
                .write_to_bytes()
                .map_err(|e| FailedOutgoingProtoPacking(e))?,
            dest: BlobDestination::Broadcast,
        }]))
    } else if msg.has_set_multicast_role() {
        let mcast = msg.get_set_multicast_role();

        if !mcast.has_assign() || !mcast.has_role() {
            Err(MissingFields)
        } else {
            match mcast.get_assign() {
                AssignOrUnassign::ASSIGN => sender_info.current_roles.insert(mcast.get_role()),
                AssignOrUnassign::UNASSIGN => sender_info.current_roles.remove(&mcast.get_role()),
            };
            let mut tlm = TopLevelMessage::new();
            tlm.set_management(msg.to_owned());

            Ok(Some(vec![OutgoingBlob {
                blob: tlm
                    .write_to_bytes()
                    .map_err(|e| FailedOutgoingProtoPacking(e))?,
                dest: BlobDestination::Broadcast,
            }]))
        }
    } else {
        unimplemented!()
    }
}

fn use_blob(sender_info: &mut ClientInfo, blob: Vec<u8>) -> ServerBlobResult<ServerUnpackBlobError>{
    use ServerUnpackBlobError::*;
    let tlm = TopLevelMessage::parse_from_bytes(&blob).map_err(|e| BadProtoBlob(e))?;

    if !tlm.has_magic() || tlm.get_magic() != TOP_LEVEL_MAGIC {
        Err(InvalidMagic)
    } else if tlm.has_management() {
        use_management_message(sender_info, tlm.get_management())
    } else if tlm.has_payload() {
        let dest = {
            use BlobDestination::*;
            if tlm.has_broadcast() {
                Broadcast
            } else if tlm.has_multicast_role() {
                Multicast(tlm.get_multicast_role())
            } else if tlm.has_unicast() {
                Unicast(tlm.get_unicast().to_owned())
            } else {
                unimplemented!()
            }
        };

        Ok(Some(vec![OutgoingBlob {
            blob: tlm
                .write_to_bytes()
                .map_err(|e| FailedOutgoingProtoPacking(e))?,
            dest,
        }]))
    } else {
        unimplemented!()
    }
}
