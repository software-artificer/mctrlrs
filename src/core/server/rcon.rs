use std::{
    fmt,
    io::{self, Read, Write},
    net,
};

use secrecy::ExposeSecret;

#[derive(thiserror::Error, Debug)]
pub enum RconError {
    #[error("Failed to connect to the Minecraft server: {0}")]
    Connect(#[source] io::Error),
    #[error("Failed to decode the message received from the Minecraft server: {0}")]
    Decode(String),
    #[error("A message payload must be less than {0} bytes, got: {1}")]
    PayloadTooBig(usize, usize),
    #[error("Failed to send a message to the Minecraft server: {0}")]
    Write(#[source] io::Error),
    #[error("Failed to read a message from the Minecraft server: {0}")]
    Read(#[source] io::Error),
    #[error("Minecraft server authentication failed")]
    AuthFail,
    #[error("Expected sequence ID {0} from the Minecraft server, got: {1}")]
    IdMismatch(i32, i32),
    #[error("Expected ID to be greater than 0, got: {0}")]
    InvalidId(i32),
    #[error("Invalid packet type received from the server. Expected {0}, got: {1}")]
    InvalidPacketType(String, String),
}

pub struct Disconnected;

pub struct Connected(net::TcpStream);

pub struct Authenticated {
    inner: Connected,
    id: i32,
}

pub struct RconClient<T> {
    state: T,
}

impl RconClient<Disconnected> {
    pub fn new() -> Self {
        Self {
            state: Disconnected,
        }
    }

    pub fn connect(self, addr: net::SocketAddr) -> Result<RconClient<Connected>, RconError> {
        let stream = net::TcpStream::connect(addr).map_err(RconError::Connect)?;

        Ok(RconClient {
            state: Connected(stream),
        })
    }
}

impl RconClient<Connected> {
    pub fn authenticate(
        mut self,
        password: secrecy::SecretString,
    ) -> Result<RconClient<Authenticated>, RconError> {
        let request = RconPacket::authentication(0, password.expose_secret().clone())?;

        self.state
            .0
            .write_all(&request.encode())
            .map_err(RconError::Write)?;

        let size = read_size(&mut self.state.0)?;
        let packet = read_packet(&mut self.state.0, size)?;

        if let RconPacketType::Command = packet.packet_type {
            match packet.id {
                -1 => Err(RconError::AuthFail),
                0 => Ok(RconClient {
                    state: Authenticated {
                        inner: self.state,
                        id: 0,
                    },
                }),
                id => Err(RconError::IdMismatch(0, id)),
            }
        } else {
            Err(RconError::InvalidPacketType(
                RconPacketType::Command.to_string(),
                packet.packet_type.to_string(),
            ))
        }
    }
}

impl RconClient<Authenticated> {
    pub fn command(&mut self, data: String) -> Result<String, RconError> {
        let id = self.id();
        self.state
            .inner
            .0
            .write_all(&RconPacket::command(id, data)?.encode())
            .map_err(RconError::Write)?;

        let size = read_size(&mut self.state.inner.0)?;
        let packet = read_packet(&mut self.state.inner.0, size)?;

        if packet.id != id {
            Err(RconError::IdMismatch(0, packet.id))
        } else if let RconPacketType::Response = packet.packet_type {
            if size == RconPacket::MAX_PACKET_SIZE {
                let new_id = self.id();
                read_fragmented(&mut self.state.inner.0, packet.payload, new_id, id)
            } else {
                Ok(packet.payload)
            }
        } else {
            Err(RconError::InvalidPacketType(
                RconPacketType::Response.to_string(),
                packet.packet_type.to_string(),
            ))
        }
    }

    fn id(&mut self) -> i32 {
        if self.state.id == i32::MAX {
            self.state.id = 1;
        } else {
            self.state.id += 1;
        }

        self.state.id
    }
}

fn read_size(stream: &mut net::TcpStream) -> Result<i32, RconError> {
    let mut buf = [0; 4];
    stream.read_exact(&mut buf).map_err(RconError::Read)?;
    let size = i32::from_le_bytes(buf);
    if !(RconPacket::MIN_PACKET_SIZE..=RconPacket::MAX_PACKET_SIZE).contains(&size) {
        Err(RconError::Decode(format!(
            "A packet size must be between {} and {} bytes long, server sent: {}",
            RconPacket::MIN_PACKET_SIZE,
            RconPacket::MAX_PACKET_SIZE,
            size
        )))
    } else {
        Ok(size)
    }
}

fn read_packet(stream: &mut net::TcpStream, size: i32) -> Result<RconPacket, RconError> {
    let mut buf = vec![0; size as usize];
    stream.read_exact(&mut buf).map_err(RconError::Read)?;

    RconPacket::decode(buf)
}

fn read_fragmented(
    stream: &mut net::TcpStream,
    mut result: String,
    new_id: i32,
    id: i32,
) -> Result<String, RconError> {
    stream
        .write_all(&RconPacket::check(new_id)?.encode())
        .map_err(RconError::Write)?;

    loop {
        let size = read_size(stream)?;
        let packet = read_packet(stream, size)?;

        if packet.id == id {
            result.push_str(&packet.payload);
        } else if packet.id == new_id {
            if let RconPacketType::Response = packet.packet_type {
                if packet.payload == "Unknown request 0" {
                    break Ok(result);
                } else {
                    break Err(RconError::InvalidPacketType(
                        RconPacketType::Response.to_string(),
                        packet.packet_type.to_string(),
                    ));
                }
            }
        } else {
            break Err(RconError::IdMismatch(new_id, packet.id));
        }
    }
}

#[derive(Debug)]
enum RconPacketType {
    Authentication,
    Command,
    Response,
}

impl fmt::Display for RconPacketType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}",
            match self {
                RconPacketType::Authentication => "authentication",
                RconPacketType::Command => "command",
                RconPacketType::Response => "response",
            }
        )
    }
}

impl IntoIterator for RconPacketType {
    type Item = u8;
    type IntoIter = std::array::IntoIter<u8, 4>;

    fn into_iter(self) -> Self::IntoIter {
        match self {
            RconPacketType::Response => 0i32.to_le_bytes().into_iter(),
            RconPacketType::Command => 2i32.to_le_bytes().into_iter(),
            RconPacketType::Authentication => 3i32.to_le_bytes().into_iter(),
        }
    }
}

impl TryFrom<i32> for RconPacketType {
    type Error = RconError;

    fn try_from(value: i32) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Response),
            2 => Ok(Self::Command),
            _ => Err(RconError::Decode(format!(
                "Expected message type to be 0, got: {}",
                value
            ))),
        }
    }
}

struct RconPacket {
    id: i32,
    packet_type: RconPacketType,
    payload: String,
}

impl RconPacket {
    const MIN_PACKET_SIZE: i32 = 10;
    const MAX_PACKET_SIZE: i32 = 4106;
    const PACKET_PAD_SIZE: i32 = 2;

    const MAX_CLIENT_PAYLOAD_SIZE: usize = 1446;

    fn authentication(id: i32, password: String) -> Result<Self, RconError> {
        Self::new(id, RconPacketType::Authentication, password)
    }

    fn command(id: i32, payload: String) -> Result<Self, RconError> {
        Self::new(id, RconPacketType::Command, payload)
    }

    fn check(id: i32) -> Result<Self, RconError> {
        Self::new(id, RconPacketType::Response, String::new())
    }

    fn new(id: i32, message_type: RconPacketType, payload: String) -> Result<Self, RconError> {
        if id < 0 {
            Err(RconError::InvalidId(id))
        } else if payload.len() > Self::MAX_CLIENT_PAYLOAD_SIZE {
            Err(RconError::PayloadTooBig(
                Self::MAX_CLIENT_PAYLOAD_SIZE,
                payload.len(),
            ))
        } else {
            Ok(Self {
                id,
                packet_type: message_type,
                payload,
            })
        }
    }

    fn encode(self) -> Vec<u8> {
        let mut bytes = vec![];
        bytes.extend(self.id.to_le_bytes());
        bytes.extend(self.packet_type);
        bytes.extend(self.payload.as_bytes());
        bytes.extend([0, 0]);

        let size = bytes.len() as i32;
        let mut packet = vec![];
        packet.extend(size.to_le_bytes());
        packet.extend(bytes);

        packet
    }

    fn decode(mut bytes: Vec<u8>) -> Result<Self, RconError> {
        if bytes.len() < Self::MIN_PACKET_SIZE as usize {
            Err(RconError::Decode(format!(
                "Expected packet length to be at least {} bytes, got: {}",
                Self::MIN_PACKET_SIZE,
                bytes.len()
            )))
        } else {
            let id: [u8; 4] = bytes.drain(0..4).as_slice().try_into().unwrap();
            let id = i32::from_le_bytes(id);

            let message_type: [u8; 4] = bytes.drain(0..4).as_slice().try_into().unwrap();
            let message_type = i32::from_le_bytes(message_type).try_into()?;

            let mut payload = String::new();

            let payload_size = bytes.len() - Self::PACKET_PAD_SIZE as usize;
            if payload_size > 0 {
                payload =
                    String::from_utf8(bytes.drain(0..payload_size).collect()).map_err(|e| {
                        RconError::Decode(format!(
                            "Failed to convert message body to a UTF-8 string: {e}"
                        ))
                    })?;
            }

            if bytes.drain(0..2).as_slice() != [0, 0] {
                Err(RconError::Decode(
                    "Missing padding at the end of the message".to_string(),
                ))
            } else {
                Ok(Self {
                    id,
                    payload,
                    packet_type: message_type,
                })
            }
        }
    }
}
