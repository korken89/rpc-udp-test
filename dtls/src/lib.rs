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

use buffer::DTlsBuffer;
use handshake::ClientHandshake;
use rand_core::{CryptoRng, RngCore};
use session::RecordNumber;

pub mod buffer;
pub(crate) mod handshake;
pub mod integers;
pub mod record;
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
pub struct DTlsConnection<Socket> {
    /// Sender/receiver of data.
    socket: Socket,
    /// TODO: Keys for client->server and server->client. Also called "key schedule".
    crypto: (),
}

impl<Socket> DTlsConnection<Socket>
where
    Socket: UdpSocket + Clone,
{
    /// Open a DTLS 1.3 client connection.
    /// This returns an active connection after handshake is completed.
    ///
    /// NOTE: This does not do timeout, it's up to the caller to give up.
    pub async fn open_client<Rng>(
        rng: &mut Rng,
        buf: &mut impl DTlsBuffer,
        socket: Socket,
    ) -> Result<Self, DTlsError<Socket>>
    where
        Rng: RngCore + CryptoRng,
    {
        // let mut handshake = ClientHandshake::new();

        // let crypto = handshake.perform(buf, &socket, rng).await?;

        let crypto = ();

        Ok(DTlsConnection { socket, crypto })
    }

    /// Open a DTLS 1.3 server connection.
    /// This returns an active connection after handshake is completed.
    ///
    /// NOTE: This does not do timeout, it's up to the caller to give up.
    pub async fn open_server<Rng>(
        rng: &mut Rng,
        buf: &mut impl DTlsBuffer,
        socket: Socket,
    ) -> Result<Self, DTlsError<Socket>>
    where
        Rng: RngCore + CryptoRng,
    {
        todo!()
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
    connection: &'a DTlsConnection<Socket>,
    record_number: RecordNumber,
}

impl<'a, Socket> DTlsSender<'a, Socket> where Socket: UdpSocket + Clone + 'a {}

/// Receiver half of a DTLS connection.
pub struct DTlsReceiver<'a, Socket> {
    connection: &'a DTlsConnection<Socket>,
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

pub mod cipher_suites {
    // use chacha20poly1305::{aead::AeadMutInPlace, ChaCha20Poly1305};
    // use digest::{core_api::BlockSizeUser, Digest, FixedOutput, OutputSizeUser, Reset};
    // use generic_array::ArrayLength;
    // use sha2::Sha256;
    // use typenum::{U12, U16};

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

    // pub trait TlsCipherSuite {
    //     const CODE_POINT: u16;
    //     type Cipher: AeadMutInPlace<NonceSize = Self::IvLen>;
    //     type KeyLen: ArrayLength;
    //     type IvLen: ArrayLength;
    //     type Hash: Digest + Reset + Clone + OutputSizeUser + BlockSizeUser + FixedOutput;
    // }

    // Aes cipher
    // pub struct Aes128GcmSha256;
    // impl TlsCipherSuite for Aes128GcmSha256 {
    //     const CODE_POINT: u16 = CipherSuite::TlsAes128GcmSha256 as u16;
    //     type Cipher = Aes128Gcm;
    //     type KeyLen = U16;
    //     type IvLen = U12;
    //     type Hash = Sha256;
    // }

    // // Chacha chipher
    // pub struct Chacha20Poly1305Sha256;
    // impl TlsCipherSuite for Chacha20Poly1305Sha256 {
    //     const CODE_POINT: u16 = CipherSuite::TlsChacha20Poly1305Sha256 as u16;
    //     type Cipher = ChaCha20Poly1305;
    //     type KeyLen = U16;
    //     type IvLen = U12;
    //     type Hash = Sha256;
    // }
}
