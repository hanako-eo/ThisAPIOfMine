use deku::prelude::*;
use uuid::Uuid;

use crate::deku_helper;

#[derive(Debug, DekuWrite)]
#[deku(endian = "endian", ctx = "endian: deku::ctx::Endian")]
pub struct PlayerData {
    #[deku(writer = "deku_helper::write_uuid(deku::writer, &self.uuid)")]
    uuid: Uuid,
    #[deku(writer = "deku_helper::write_str(deku::writer, &self.nickname)")]
    nickname: String,
    #[deku(writer = "deku_helper::write_vec_str(deku::writer, &self.permissions)")]
    permissions: Vec<String>,
}

impl PlayerData {
    pub fn new(uuid: Uuid, nickname: String, permissions: Vec<String>) -> Self {
        Self {
            uuid,
            nickname,
            permissions,
        }
    }
}
