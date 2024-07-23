use std::error::Error as StdError;
use std::ops::Deref;

use actix_web::web::BytesMut;
use base64::prelude::*;
use rand_core::{CryptoRng, RngCore};
use tokio_postgres::types::{Format, IsNull, ToSql, Type};

use crate::errors::Result;

#[repr(transparent)]
#[derive(Debug)]
pub struct Token(Box<str>);

impl Token {
    pub fn generate<R>(mut rng: R) -> Result<Self>
    where
        R: CryptoRng + RngCore,
    {
        let mut key = [0u8; 32];
        rng.try_fill_bytes(&mut key)?;

        Ok(Self(BASE64_STANDARD.encode(key).into_boxed_str()))
    }
}

impl Deref for Token {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.0.as_ref()
    }
}

impl ToSql for Token {
    #[inline]
    fn accepts(ty: &Type) -> bool
    where
        Self: Sized,
    {
        <&str as ToSql>::accepts(ty)
    }

    #[inline]
    fn encode_format(&self, ty: &Type) -> Format {
        self.0.encode_format(ty)
    }

    #[inline]
    fn to_sql(
        &self,
        ty: &Type,
        out: &mut BytesMut,
    ) -> Result<IsNull, Box<dyn StdError + Sync + Send>>
    where
        Self: Sized,
    {
        self.0.to_sql(ty, out)
    }

    #[inline]
    fn to_sql_checked(
        &self,
        ty: &Type,
        out: &mut BytesMut,
    ) -> Result<IsNull, Box<dyn StdError + Sync + Send>> {
        self.0.to_sql_checked(ty, out)
    }
}
