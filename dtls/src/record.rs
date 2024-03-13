use rand_core::{CryptoRng, RngCore};

use crate::{
    buffer::DTlsBuffer,
    handshake::{ClientHandshake, ClientHello},
    integers::U48,
    DTlsError, UdpSocket,
};

#[derive(Copy, Clone, Debug, defmt::Format, PartialEq, Eq, Hash)]
pub enum Encryption {
    Enabled,
    Disabled,
}

pub enum ClientRecord {
    Handshake(ClientHandshake, Encryption),
    ChangeCipherSpec(/* ChangeCipherSpec */ (), Encryption),
    Alert(/* Alert, */ (), Encryption),
    Heartbeat((), Encryption),
    Ack((), Encryption),
    ApplicationData(/* &'a [u8] */),
}

impl ClientRecord {
    pub fn content_type(&self) -> ContentType {
        match self {
            ClientRecord::Handshake(_, Encryption::Disabled) => ContentType::Handshake,
            ClientRecord::ChangeCipherSpec(_, Encryption::Disabled) => {
                ContentType::ChangeCipherSpec
            }
            ClientRecord::Alert(_, Encryption::Disabled) => ContentType::Alert,
            ClientRecord::Heartbeat(_, Encryption::Disabled) => ContentType::Heartbeat,
            ClientRecord::Ack(_, Encryption::Disabled) => ContentType::Ack,
            // All encrypted communication is marked as `ApplicationData`.
            ClientRecord::Handshake(_, Encryption::Enabled) => ContentType::ApplicationData,
            ClientRecord::ChangeCipherSpec(_, Encryption::Enabled) => ContentType::ApplicationData,
            ClientRecord::Alert(_, Encryption::Enabled) => ContentType::ApplicationData,
            ClientRecord::Heartbeat(_, Encryption::Enabled) => ContentType::ApplicationData,
            ClientRecord::Ack(_, Encryption::Enabled) => ContentType::ApplicationData,
            ClientRecord::ApplicationData() => ContentType::ApplicationData,
        }
    }

    /// Create a client hello handshake.
    pub fn client_hello<Rng>(rng: &mut Rng) -> Self
    where
        Rng: RngCore + CryptoRng,
    {
        ClientRecord::Handshake(
            ClientHandshake::ClientHello(ClientHello::new(rng)),
            Encryption::Disabled,
        )
    }

    /// Encode the record into a buffer.
    pub fn encode<S: UdpSocket>(
        buf: &mut impl DTlsBuffer,
        record: &ClientRecord,
    ) -> Result<(), DTlsError<S>> {
        buf.push_u8(record.content_type() as u8)
            .map_err(|_| DTlsError::InsufficientSpace)?;

        Ok(())
    }
}

// pub struct DTlsRecord {}
type ProtocolVersion = [u8; 2];
const LEGACY_DTLS_VERSION: ProtocolVersion = [254, 253];

pub struct DTlsPlaintext {
    type_: ContentType,
    legacy_record_version: ProtocolVersion,
    epoch: u16,
    sequence_number: U48,
    length: u16,
    // plaintext:
}

pub struct DTlsInnerPlaintext {}

pub struct DTlsCiphertext {}

/// TLS content type. RFC 9147 - Appendix A.1
#[derive(Debug, defmt::Format)]
pub enum ContentType {
    ChangeCipherSpec = 20,
    Alert = 21,
    Handshake = 22,
    ApplicationData = 23,
    Heartbeat = 24,
    // Tls12Cid = 25,
    Ack = 26,
}

impl ContentType {
    pub fn of(num: u8) -> Option<Self> {
        match num {
            20 => Some(Self::ChangeCipherSpec),
            21 => Some(Self::Alert),
            22 => Some(Self::Handshake),
            23 => Some(Self::ApplicationData),
            _ => None,
        }
    }
}
