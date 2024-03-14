use crate::{buffer::DTlsBuffer, integers::U24, record::LEGACY_DTLS_VERSION, DTlsError, UdpSocket};
use rand_core::{CryptoRng, RngCore};
use x25519_dalek::{EphemeralSecret, PublicKey};

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
    pub fn encode(&self, buf: &mut impl DTlsBuffer) -> Result<(), ()> {
        todo!()
    }
}

pub struct ClientHello {
    // legacy_version: ProtocolVersion,
    random: Random,
    // legacy_session_id: u8[0..32]
    // legacy_cookie: u8[0..2^8-2]
    // TODO: In the future we can support more than one.
    // cipher_suites: [CipherSuite; 1],
    // legacy_compression_mesthods: u8[1..2^8-1]
    // extensions: &[Extensions]
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
        let pubkey = PublicKey::from(&self.secret);

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

pub mod extensions {
    //! Extensions we probably want:
    //!
    //! * psk_key_exchange_modes (this looks interesting)
    //! * key_share
    //! * heartbeat
    //! * pre_shared_key
    //!
    //! embedded-tls has these as well:
    //!
    //! * signature_algorithms
    //! * supported_groups
    //! * server_name (this looks interesting)
    //! * supported_versions (only DTLS 1.3), not needed it turns out
    //!
    //! All this is defined in RFC 8446 (TLS 1.3) at Page 37.

    use heapless::Vec;

    use crate::buffer::{AllocSliceHandle, DTlsBuffer};

    #[derive(Clone, Debug, PartialOrd, PartialEq)]
    #[cfg_attr(feature = "defmt", derive(defmt::Format))]
    pub enum ClientExtensions<'a> {
        PskKeyExchangeModes(PskKeyExchangeModes),
        KeyShare(KeyShareEntry<'a>),
        PreSharedKey(OfferedPsks<'a>),
        // ServerName { // Not sure we need this.
        //     server_name: &'a str,
        // },
        // Heartbeat { // Not sure we need this.
        //     mode: HeartbeatMode,
        // },
        // SupportedGroups {
        //     supported_groups: Vec<NamedGroup, 16>,
        // },
        // SupportedVersions {
        //     versions: ProtocolVersions,
        // },
        // SignatureAlgorithms {
        //     supported_signature_algorithms: Vec<SignatureScheme, 16>,
        // },
        // SignatureAlgorithmsCert {
        //     supported_signature_algorithms: Vec<SignatureScheme, 16>,
        // },
        // MaxFragmentLength(MaxFragmentLength),
    }

    impl<'a> ClientExtensions<'a> {
        /// Encode a client extension.
        /// Encode the offered pre-shared keys. Returns a handle to write the binders if needed.
        pub fn encode(&self, buf: &mut impl DTlsBuffer) -> Result<Option<AllocSliceHandle>, ()> {
            // ...
            todo!()
        }

        fn extension_type(&self) -> ExtensionType {
            match self {
                ClientExtensions::PskKeyExchangeModes { .. } => ExtensionType::PskKeyExchangeModes,
                ClientExtensions::KeyShare(_) => ExtensionType::KeyShare,
                ClientExtensions::PreSharedKey(_) => ExtensionType::PreSharedKey,
            }
        }
    }

    /// Pre-Shared Key Exchange Modes.
    #[derive(Clone, Debug, PartialOrd, PartialEq)]
    #[cfg_attr(feature = "defmt", derive(defmt::Format))]
    pub struct PskKeyExchangeModes {
        ke_modes: Vec<PskKeyExchangeMode, 4>,
    }

    impl PskKeyExchangeModes {
        /// Encode a `psk_key_exchange_modes` extension.
        pub fn encode(&self, buf: &mut impl DTlsBuffer) -> Result<(), ()> {
            buf.push_u8(self.ke_modes.len() as u8)?;
            for mode in &self.ke_modes {
                buf.push_u8(*mode as u8)?;
            }

            Ok(())
        }
    }

    /// The `key_share` extension contains the endpoint’s cryptographic parameters.
    #[derive(Clone, Debug, PartialOrd, PartialEq)]
    #[cfg_attr(feature = "defmt", derive(defmt::Format))]
    pub struct KeyShareEntry<'a> {
        group: NamedGroup,
        opaque: &'a [u8],
    }

    impl<'a> KeyShareEntry<'a> {
        /// Encode a `key_share` extension.
        pub fn encode(&self, buf: &mut impl DTlsBuffer) -> Result<(), ()> {
            buf.push_u16_be(2 + 2 + self.opaque.len() as u16)?;

            // one key-share
            buf.push_u16_be(self.group as u16)?;
            buf.push_u16_be(self.opaque.len() as u16)?;
            buf.extend_from_slice(self.opaque)
        }
    }

    /// The pre-shared keys the client can offer to use.
    #[derive(Clone, Debug, PartialOrd, PartialEq)]
    #[cfg_attr(feature = "defmt", derive(defmt::Format))]
    pub struct OfferedPsks<'a> {
        /// List of identities that can be used. Ticket age is set to 0.
        identities: &'a [PskIdentity<'a>],
        /// Size of the binder hash.
        hash_size: usize,
    }

    impl<'a> OfferedPsks<'a> {
        /// Encode the offered pre-shared keys. Returns a handle to write the binders.
        pub fn encode(&self, buf: &mut impl DTlsBuffer) -> Result<AllocSliceHandle, ()> {
            let ident_len = self
                .identities
                .iter()
                .map(|ident| ident.identity.len() + 4 + 2)
                .sum::<usize>();

            // Length.
            buf.push_u16_be(ident_len as u16)?;

            // Each identity.
            for identity in self.identities {
                identity.encode(buf)?;
            }

            // Allocate space for binders and return it for future use.
            let binders_len = (1 + self.hash_size) * self.identities.len();
            buf.alloc_slice(binders_len)
        }
    }

    /// Pre-shared key identity payload.
    #[derive(Clone, Debug, PartialOrd, PartialEq)]
    #[cfg_attr(feature = "defmt", derive(defmt::Format))]
    pub struct PskIdentity<'a> {
        /// A label for a key. For instance, a ticket (as defined in Appendix B.3.4) or a label
        /// for a pre-shared key established externally.
        identity: &'a [u8],
    }

    impl<'a> PskIdentity<'a> {
        /// Encode a pre-shared key identity into the buffer.
        pub fn encode(&self, buf: &mut impl DTlsBuffer) -> Result<(), ()> {
            // Encode length.
            buf.push_u16_be(self.identity.len() as u16)?;

            // Encode identity.
            buf.extend_from_slice(self.identity)?;

            // Encode ticket age.
            buf.push_u32_be(0)
        }
    }

    /// Heartbeat mode.
    #[derive(Copy, Clone, Debug, PartialOrd, PartialEq)]
    #[cfg_attr(feature = "defmt", derive(defmt::Format))]
    pub enum HeartbeatMode {
        PeerAllowedToSend = 1,
        PeerNotAllowedToSend = 2,
    }

    /// Pre-Shared Key Exchange Modes (RFC 8446, 4.2.9)
    #[repr(u8)]
    #[derive(Copy, Clone, Debug, PartialOrd, PartialEq)]
    #[cfg_attr(feature = "defmt", derive(defmt::Format))]
    pub enum PskKeyExchangeMode {
        ///  PSK-only key establishment. In this mode, the server MUST NOT supply a `key_share` value.
        PskKe = 0,
        /// PSK with (EC)DHE key establishment. In this mode, the client and server MUST supply
        /// `key_share` values.
        PskDheKe = 1,
    }

    /// Named groups which the client supports for key exchange.
    #[repr(u16)]
    #[derive(Copy, Clone, Debug, PartialOrd, PartialEq)]
    #[cfg_attr(feature = "defmt", derive(defmt::Format))]
    pub enum NamedGroup {
        // Elliptic Curve Groups (ECDHE)
        Secp256r1 = 0x0017,
        Secp384r1 = 0x0018,
        Secp521r1 = 0x0019,
        X25519 = 0x001D,
        X448 = 0x001E,

        // Finite Field Groups (DHE)
        Ffdhe2048 = 0x0100,
        Ffdhe3072 = 0x0101,
        Ffdhe4096 = 0x0102,
        Ffdhe6144 = 0x0103,
        Ffdhe8192 = 0x0104,
    }

    impl NamedGroup {
        pub fn of(num: u16) -> Option<NamedGroup> {
            match num {
                0x0017 => Some(Self::Secp256r1),
                0x0018 => Some(Self::Secp384r1),
                0x0019 => Some(Self::Secp521r1),
                0x001D => Some(Self::X25519),
                0x001E => Some(Self::X448),
                0x0100 => Some(Self::Ffdhe2048),
                0x0101 => Some(Self::Ffdhe3072),
                0x0102 => Some(Self::Ffdhe4096),
                0x0103 => Some(Self::Ffdhe6144),
                0x0104 => Some(Self::Ffdhe8192),
                _ => None,
            }
        }
    }

    /// TLS ExtensionType Values registry.
    #[repr(u16)]
    #[derive(Copy, Clone, Debug, PartialOrd, PartialEq)]
    #[cfg_attr(feature = "defmt", derive(defmt::Format))]
    pub enum ExtensionType {
        ServerName = 0,
        MaxFragmentLength = 1,
        StatusRequest = 5,
        SupportedGroups = 10,
        SignatureAlgorithms = 13,
        UseSrtp = 14,
        Heatbeat = 15,
        ApplicationLayerProtocolNegotiation = 16,
        SignedCertificateTimestamp = 18,
        ClientCertificateType = 19,
        ServerCertificateType = 20,
        Padding = 21,
        PreSharedKey = 41,
        EarlyData = 42,
        SupportedVersions = 43,
        Cookie = 44,
        PskKeyExchangeModes = 45,
        CertificateAuthorities = 47,
        OidFilters = 48,
        PostHandshakeAuth = 49,
        SignatureAlgorithmsCert = 50,
        KeyShare = 51,
    }
}
