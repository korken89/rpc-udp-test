//! A DTLS 1.3 PSK implementation.
//!
//!
//!
//!
//!
//! Heavily inspired by [`embedded-tls`].
//! [`embedded-tls`]: https://github.com/drogue-iot/embedded-tls

#![no_std]
#![allow(async_fn_in_trait)]

use buffer::TlsBuffer;
use handshake::Handshake;
use heapless::Vec;
use p256_cortex_m4::SecretKey;
use rand_core::{CryptoRng, RngCore};
use session::RecordNumber;

pub(crate) mod buffer;
pub(crate) mod handshake;
pub mod session;

// The TLS cake
//
// 1. Record layer (fragmentation and such)
// 2. The payload (Handshake, ChangeCipherSpec, Alert, ApplicationData)

#[derive(Debug, Copy, Clone, defmt::Format)]
pub enum DTlsError<Socket: UdpSocket> {
    /// The backing buffer ran out of space.
    InsufficientSpace,
    UdpSend(Socket::SendError),
    UdpRecv(Socket::ReceiveError),
}

/// UDP socket trait, send and receives from/to a single endpoint.
///
/// This means on `std` that it cannot be implemented directly on a socket, but probably a
/// sender/receiver pair which splits the incoming packets based on IP or similar.
pub trait UdpSocket {
    /// Error type for sending.
    type SendError: defmt::Format;
    /// Error type for receiving.
    type ReceiveError: defmt::Format;

    /// Send a UDP packet.
    async fn send(&self, buf: &[u8]) -> Result<(), Self::SendError>;
    /// Receive a UDP packet.
    async fn recv(&self, buf: &mut [u8]) -> Result<(), Self::ReceiveError>;
}

// TODO: How to select between server and client? Typestate, flag or two separate structs?
/// A DTLS 1.3 connection.
pub struct DTlsConnection<'a, Socket> {
    /// Sender/receiver of data.
    socket: Socket,
    /// TODO: Keys for client->server and server->client. Also called "key schedule".
    crypto: (),
    /// Backing buffer.
    record_buf: &'a mut [u8],
}

impl<'a, Socket> DTlsConnection<'a, Socket>
where
    Socket: UdpSocket + Clone + 'a,
{
    /// Open a DTLS 1.3 connection. This returns an active connection after handshake is completed.
    ///
    /// NOTE: This does not do timeout, it's up to the caller to give up.
    pub async fn open<Rng>(
        mut rng: Rng,
        buf: &'a mut [u8],
        socket: Socket,
    ) -> Result<Self, DTlsError<Socket>>
    where
        Rng: RngCore + CryptoRng,
    {
        let mut buffer = TlsBuffer::new(buf);

        let mut handshake = Handshake::new();

        Ok(DTlsConnection {
            socket,
            crypto: (),
            record_buf: buf,
        })
    }

    // TODO: Seems like this is the interface we want in the end.
    pub async fn split(&mut self) -> (DTlsSender<'_, Socket>, DTlsReceiver<'_, Socket>) {
        (
            DTlsSender {
                connection: self,
                record_number: RecordNumber::new(),
            },
            DTlsReceiver {
                connection: self,
                record_number: RecordNumber::new(),
            },
        )
    }
}

/// Sender half of a DTLS connection.
pub struct DTlsSender<'a, Socket> {
    connection: &'a DTlsConnection<'a, Socket>,
    record_number: RecordNumber,
}

impl<'a, Socket> DTlsSender<'a, Socket> where Socket: UdpSocket + Clone + 'a {}

/// Receiver half of a DTLS connection.
pub struct DTlsReceiver<'a, Socket> {
    connection: &'a DTlsConnection<'a, Socket>,
    record_number: RecordNumber,
}

impl<'a, Socket> DTlsReceiver<'a, Socket> where Socket: UdpSocket + Clone + 'a {}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn open_connection() {
        loop {
            // Data pump
        }
    }
}

// ------------------------------------------------------------------------

type Random = [u8; 32];

const LEGACY_TLS_VERSION: u16 = 0x0303;

#[derive(Debug, Copy, Clone, defmt::Format)]
pub enum HandshakeType {
    ClientHello = 1,
    ServerHello = 2,
    NewSessionTicket = 4,
    EndOfEarlyData = 5,
    EncryptedExtensions = 8,
    Certificate = 11,
    CertificateRequest = 13,
    CertificateVerify = 15,
    Finished = 20,
    KeyUpdate = 24,
    MessageHash = 254,
}

pub mod cipher_suites {
    use chacha20poly1305::{aead::AeadMutInPlace, ChaCha20Poly1305};
    use digest::{core_api::BlockSizeUser, Digest, FixedOutput, OutputSizeUser, Reset};
    use generic_array::ArrayLength;
    use sha2::Sha256;
    use typenum::{U12, U16};

    /// Represents a TLS 1.3 cipher suite
    #[derive(Copy, Clone, Debug, defmt::Format)]
    pub enum CipherSuite {
        // TlsAes128GcmSha256 = 0x1301,
        // TlsAes256GcmSha384 = 0x1302,
        TlsChacha20Poly1305Sha256 = 0x1303,
        // TlsAes128CcmSha256 = 0x1304,
        // TlsAes128Ccm8Sha256 = 0x1305,
        // TlsPskAes128GcmSha256 = 0x00A8,
    }

    impl CipherSuite {
        pub fn of(num: u16) -> Option<Self> {
            match num {
                // 0x1301 => Some(Self::TlsAes128GcmSha256),
                // 0x1302 => Some(Self::TlsAes256GcmSha384),
                0x1303 => Some(Self::TlsChacha20Poly1305Sha256),
                // 0x1304 => Some(Self::TlsAes128CcmSha256),
                // 0x1305 => Some(Self::TlsAes128Ccm8Sha256),
                // 0x00A8 => Some(Self::TlsPskAes128GcmSha256),
                // 0xCC,0xAB	TLS_PSK_WITH_CHACHA20_POLY1305_SHA256
                // 0xCC,0xAC	TLS_ECDHE_PSK_WITH_CHACHA20_POLY1305_SHA256
                _ => None,
            }
        }
    }

    pub trait TlsCipherSuite {
        const CODE_POINT: u16;
        type Cipher: AeadMutInPlace<NonceSize = Self::IvLen>;
        type KeyLen: ArrayLength;
        type IvLen: ArrayLength;

        type Hash: Digest + Reset + Clone + OutputSizeUser + BlockSizeUser + FixedOutput;
    }

    // Aes cipher
    // pub struct Aes128GcmSha256;
    // impl TlsCipherSuite for Aes128GcmSha256 {
    //     const CODE_POINT: u16 = CipherSuite::TlsAes128GcmSha256 as u16;
    //     type Cipher = Aes128Gcm;
    //     type KeyLen = U16;
    //     type IvLen = U12;
    //
    //     type Hash = Sha256;
    // }

    // Chacha chipher
    pub struct Chacha20Poly1305Sha256;
    impl TlsCipherSuite for Chacha20Poly1305Sha256 {
        const CODE_POINT: u16 = CipherSuite::TlsChacha20Poly1305Sha256 as u16;
        type Cipher = ChaCha20Poly1305;
        type KeyLen = U16;
        type IvLen = U12;

        type Hash = Sha256;
    }
}

pub struct ClientHello {
    random: Random,
    secret: SecretKey,
}

impl ClientHello {
    pub fn new<Rng>(mut rng: Rng) -> Self
    where
        Rng: RngCore + CryptoRng,
    {
        let mut random = [0; 32];
        rng.fill_bytes(&mut random);

        let key = SecretKey::from_bytes(&random).unwrap();

        rng.fill_bytes(&mut random);

        Self {
            random,
            secret: key,
        }
    }

    pub fn encode<const N: usize>(&self, buf: &mut Vec<u8, N>) -> Result<(), ()> {
        let pubkey = self.secret.public_key();
        let pubkey = pubkey.to_compressed_sec1_bytes();

        buf.extend_from_slice(&LEGACY_TLS_VERSION.to_be_bytes())?;

        buf.extend_from_slice(&self.random)?;

        // Session ID
        buf.push(0).map_err(|_| ())?;

        // compression methods, 1 byte of 0
        buf.push(1).map_err(|_| ())?;
        buf.push(0).map_err(|_| ())?;

        Ok(())
    }
}

pub fn server_hello(output: &mut Vec<u8, 512>) {
    //
}
