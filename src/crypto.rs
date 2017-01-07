//! Ergonomicish wrapper, for now, around `ring::aead::AES_128_GCM`
//!
//! Ideally this can be changed to use something like a pure Rust NaCl later.

use ring::aead;
use ring::aead::{SealingKey, OpeningKey};
use ring::rand::SystemRandom;
use std::default::Default;
use std::error::Error;
use std::io::Write;

#[derive(Serialize, Deserialize, Debug, Default)]
pub struct Key {
    pub bytes: [u8; 16],
}

impl Key {
    fn for_sealing(&self) -> SealingKey {
        SealingKey::new(&aead::AES_128_GCM, &self.bytes).unwrap()
    }

    fn for_opening(&self) -> OpeningKey {
        OpeningKey::new(&aead::AES_128_GCM, &self.bytes).unwrap()
    }

    pub fn new() -> Result<Key, Box<Error>> {
        let mut key: Key = Default::default();
        let random = SystemRandom::new();
        random.fill(&mut key.bytes)?;
        Ok(key)
    }

    pub fn from_bytes(bytes: &[u8]) -> Key {
        let mut key: Key = Default::default();
        (&mut key.bytes[..]).write(bytes).unwrap();
        key
    }
}

#[derive(Serialize, Deserialize, Debug, Default)]
pub struct Nonce {
    bytes: [u8; 12],
}

impl Nonce {
    fn new() -> Result<Nonce, Box<Error>> {
        let mut nonce: Nonce = Default::default();
        let random = SystemRandom::new();
        random.fill(&mut nonce.bytes)?;
        Ok(nonce)
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Sealed {
    nonce: Nonce,
    pub ciphertext: Vec<u8>,
}

impl Sealed {
    pub fn open(self: &Sealed, key: &Key) -> Result<Vec<u8>, Box<Error>> {
        let mut bytes = Vec::with_capacity(self.ciphertext.len());
        bytes.extend(self.ciphertext.iter());
        let realsize = aead::open_in_place(&key.for_opening(),
                                           &self.nonce.bytes,
                                           0,
                                           bytes.as_mut_slice(),
                                           &[])?;
        bytes.resize(realsize, 0);
        Ok(bytes)
    }
}

pub fn seal(key: &Key, plaintext: &[u8]) -> Result<Sealed, Box<Error>> {
    let nonce = Nonce::new()?;
    let suffix_capacity = aead::AES_128_GCM.max_overhead_len();
    let maxsize = plaintext.len() + suffix_capacity;
    let mut bytes = Vec::with_capacity(maxsize);
    bytes.extend(plaintext.iter());
    bytes.resize(maxsize, 0);
    let realsize = aead::seal_in_place(&key.for_sealing(),
                                       &nonce.bytes,
                                       bytes.as_mut_slice(),
                                       suffix_capacity,
                                       &[])?;
    bytes.resize(realsize, 0);
    Ok(Sealed {
        nonce: nonce,
        ciphertext: bytes,
    })
}
