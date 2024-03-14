use rand_core::{CryptoRng, RngCore};

use crate::{
    buffer::{AllocU16Handle, DTlsBuffer},
    handshake::{ClientHandshake, ClientHello},
    integers::{self, U48},
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
    pub fn encode<S: UdpSocket>(&self, buf: &mut impl DTlsBuffer) -> Result<(), DTlsError<S>> {
        let header = DTlsPlaintextHeader {
            type_: self.content_type(),
            sequence_number: 0.into(),
        };

        // Create record header.
        let record_length_marker = header
            .encode(buf)
            .map_err(|_| DTlsError::InsufficientSpace)?;

        // Fill in handshake.

        Ok(())
    }

    fn content_type(&self) -> ContentType {
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
}

/// Protocol version definition.
pub type ProtocolVersion = [u8; 2];

/// Value used for protocol version in DTLS 1.3.
pub const LEGACY_DTLS_VERSION: ProtocolVersion = [254, 253];

pub struct DTlsPlaintextHeader {
    type_: ContentType,
    // legacy_record_version: ProtocolVersion,
    // epoch: u16,
    sequence_number: U48,
    // length: u16, // we don't know this
    // fragment: opaque[length]
}

impl DTlsPlaintextHeader {
    fn encode(&self, buf: &mut impl DTlsBuffer) -> Result<AllocU16Handle, ()> {
        buf.push_u8(self.type_ as u8)?;
        buf.extend_from_slice(&LEGACY_DTLS_VERSION)?;
        buf.push_u16_be(0)?;
        buf.push_u48_be(self.sequence_number)?;
        buf.alloc_u16()
    }
}

pub struct DTlsInnerPlaintext {}

pub struct DTlsCiphertext {}

/// TLS content type. RFC 9147 - Appendix A.1
#[derive(Copy, Clone, Debug, PartialOrd, PartialEq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
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
