use crate::{buffer::DTlsBuffer, integers::U24, DTlsError, UdpSocket};
use rand_core::{CryptoRng, RngCore};
use x25519_dalek::EphemeralSecret;

type Random = [u8; 32];
type CipherSuite = [u8; 2];

pub enum ClientHandshake {
    ClientHello(ClientHello),
    Finished(Finished<64>), // TODO: 64 should not be hardcoded.
}

impl ClientHandshake {
    // /// Perform DTLS 1.3 handshake.
    // pub async fn perform<Socket, Rng>(
    //     &mut self,
    //     buffer: &mut impl DTlsBuffer,
    //     socket: &Socket,
    //     rng: &mut Rng,
    // ) -> Result<(), DTlsError<Socket>>
    // where
    //     Socket: UdpSocket,
    //     Rng: RngCore + CryptoRng,
    // {
    //     // TODO: Send client hello
    //     let client_hello = ClientHello::new(rng);

    //     // TODO: Receive server hello

    //     // TODO: Calculate cryptographic secrets (do verification?)

    //     // TODO: Should return the cryptographic stuff for later use as application data.
    //     todo!()
    // }
}

pub struct ServerHandshake {}

impl ServerHandshake {
    // /// Perform DTLS 1.3 handshake.
    // pub async fn perform<Socket, Rng>(
    //     &mut self,
    //     buffer: &mut impl DTlsBuffer,
    //     socket: &Socket,
    //     rng: &mut Rng,
    // ) -> Result<(), DTlsError<Socket>>
    // where
    //     Socket: UdpSocket,
    //     Rng: RngCore + CryptoRng,
    // {
    //     todo!()
    // }
}

// --------------------------------------------------------------------------
//
// TODO: This below should be its own files most likely. This will get large.
//
// --------------------------------------------------------------------------

#[repr(u8)]
pub enum HandshakeType {
    ClientHello(ClientHello) = 1,
    ServerHello = 2,
    NewSessionTicket = 4,
    EndOfEarlyData = 5,
    EncryptedExtensions = 8,
    RequestConnectionID = 9,
    NewConnectionID = 10,
    Certificate = 11,
    CertificateRequest = 13,
    CertificateVerify = 15,
    Finished = 20,
    KeyUpdate = 24,
    MessageHash = 254,
}

pub struct Handshake {
    // msg_type: HandshakeType (self.body as u8)
    length: U24,
    message_seq: u16,
    fragment_offset: U24,
    fragment_length: U24,
    body: HandshakeType,
}

impl Handshake {
    pub fn encode(&self, buf: &mut impl DTlsBuffer) -> Result<(), ()> {}
}

pub struct ClientHello {
    random: Random,
    secret: EphemeralSecret,
}

impl ClientHello {
    pub fn new<Rng>(rng: &mut Rng) -> Self
    where
        Rng: RngCore + CryptoRng,
    {
        let mut random = [0; 32];
        rng.fill_bytes(&mut random);

        let key = EphemeralSecret::random_from_rng(rng);

        Self {
            random,
            secret: key,
        }
    }

    pub fn encode(&self, buf: &mut impl DTlsBuffer) -> Result<(), ()> {
        let pubkey = self.secret.public_key();
        let pubkey = pubkey.to_compressed_sec1_bytes();

        buf.extend_from_slice(&LEGACY_DTLS_VERSION)?;

        buf.extend_from_slice(&self.random)?;

        // Session ID
        buf.push_u8(0)?;

        // compression methods, 1 byte of 0
        buf.push_u8(1)?;
        buf.push_u8(0)?;

        Ok(())
    }
}

pub struct Finished<const HASH_LEN: usize> {
    pub verify: [u8; HASH_LEN],
    // pub hash: Option<[u8; 1]>,
}

pub fn server_hello(buf: &mut impl DTlsBuffer) {
    //
}
