use chacha20poly1305::aead::{AeadCore, AeadMutInPlace, KeyInit, OsRng};
use chacha20poly1305::XChaCha20Poly1305;
use deku::prelude::*;
use rand_core::{CryptoRng, RngCore};
use serde::Serialize;
use serde_with::{base64::Base64, serde_as};
use std::mem::size_of;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use uuid::Uuid;

use crate::errors::Result;

// size_of will give the correct size of a tag (16)
const XCHACHA20POLY1305_IETF_ABYTES: usize = size_of::<chacha20poly1305::Tag>();
const TOKEN_VERSION: u32 = 1;

#[serde_as]
#[derive(Debug, Serialize)]
pub struct GameEncryptionKeys {
    #[serde_as(as = "Base64")]
    pub client_to_server: chacha20poly1305::Key,
    #[serde_as(as = "Base64")]
    pub server_to_client: chacha20poly1305::Key,
}

impl GameEncryptionKeys {
    pub fn generate<R>(rng: R) -> Self
    where
        R: CryptoRng + RngCore + Copy,
    {
        Self {
            client_to_server: XChaCha20Poly1305::generate_key(rng),
            server_to_client: XChaCha20Poly1305::generate_key(rng),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct GameServerAddress {
    pub address: String,
    pub port: u16,
}

#[derive(Debug, DekuWrite)]
#[deku(endian = "little")]
pub struct GameConnectionTokenAdditionalData {
    pub token_version: u32,
    pub expire_timestamp: u64,
    #[deku(writer = "deku_helper_write_key(deku::writer, &self.client_to_server_key)")]
    pub client_to_server_key: chacha20poly1305::Key,
    #[deku(writer = "deku_helper_write_key(deku::writer, &self.server_to_client_key)")]
    pub server_to_client_key: chacha20poly1305::Key,
}

#[serde_as]
#[derive(Debug, Serialize)]
pub struct GameConnectionToken {
    token_version: u32,
    #[serde_as(as = "Base64")]
    token_nonce: chacha20poly1305::XNonce,
    creation_timestamp: u64,
    expire_timestamp: u64,
    encryption_keys: GameEncryptionKeys,
    game_server: GameServerAddress,
    #[serde_as(as = "Base64")]
    private_token_data: Vec<u8>,
}

impl GameConnectionToken {
    pub fn generate(
        token_key: chacha20poly1305::Key,
        duration: Duration,
        server_address: GameServerAddress,
        private_token: GameConnectionTokenPrivate,
    ) -> Result<Self> {
        let timestamp = SystemTime::now().duration_since(UNIX_EPOCH)?;

        let encryption_keys = GameEncryptionKeys::generate(OsRng);

        let expire_timestamp = timestamp + duration;

        let additional_data = GameConnectionTokenAdditionalData {
            token_version: TOKEN_VERSION,
            expire_timestamp: expire_timestamp.as_secs(),
            client_to_server_key: encryption_keys.client_to_server,
            server_to_client_key: encryption_keys.server_to_client,
        };

        let additional_data_bytes = additional_data.to_bytes()?;

        let nonce = XChaCha20Poly1305::generate_nonce(OsRng);

        let mut private_token_bytes = private_token.to_bytes()?;
        private_token_bytes.resize(private_token_bytes.len() + XCHACHA20POLY1305_IETF_ABYTES, 0);

        let mut cipher = XChaCha20Poly1305::new(&token_key);
        cipher
            .encrypt_in_place(
                &nonce,
                additional_data_bytes.as_slice(),
                &mut private_token_bytes,
            )
            .unwrap();

        Ok(Self {
            token_version: TOKEN_VERSION,
            token_nonce: nonce,
            creation_timestamp: timestamp.as_secs(),
            expire_timestamp: expire_timestamp.as_secs(),
            encryption_keys,
            game_server: server_address,
            private_token_data: private_token_bytes,
        })
    }
}

#[derive(Debug, DekuWrite)]
#[deku(endian = "endian", ctx = "endian: deku::ctx::Endian")]
pub struct GamePlayerData {
    #[deku(writer = "deku_helper_write_uuid(deku::writer, &self.uuid)")]
    uuid: Uuid,
    #[deku(writer = "deku_helper_write_str(deku::writer, &self.nickname)")]
    nickname: String,
}

impl GamePlayerData {
    pub fn generate(uuid: Uuid, nickname: String) -> Self {
        Self { uuid, nickname }
    }
}

#[derive(Debug, DekuWrite)]
#[deku(endian = "little")]
pub struct GameConnectionTokenPrivate {
    #[deku(writer = "deku_helper_write_str(deku::writer, &self.api_token)")]
    api_token: String,
    #[deku(writer = "deku_helper_write_str(deku::writer, &self.api_url)")]
    api_url: String,
    player_data: GamePlayerData,
}

impl GameConnectionTokenPrivate {
    pub fn generate(
        game_api_url: String,
        game_api_token: String,
        player_data: GamePlayerData,
    ) -> Self {
        Self {
            api_token: game_api_token,
            api_url: game_api_url,
            player_data,
        }
    }
}

fn deku_helper_write_key<W: std::io::Write>(
    writer: &mut Writer<W>,
    value: &chacha20poly1305::Key,
) -> Result<(), DekuError> {
    let str_bytes = value.as_slice();
    str_bytes.to_writer(writer, ())
}

fn deku_helper_write_str<W: std::io::Write>(
    writer: &mut Writer<W>,
    value: &str,
) -> Result<(), DekuError> {
    let str_bytes = value.as_bytes();
    let str_len = str_bytes.len() as u32;
    str_len.to_writer(writer, ())?;
    str_bytes.to_writer(writer, ())
}

fn deku_helper_write_uuid<W: std::io::Write>(
    writer: &mut Writer<W>,
    value: &Uuid,
) -> Result<(), DekuError> {
    let str = value.to_bytes_le();
    str.to_writer(writer, ())
}
