use deku::writer::Writer;
use deku::{DekuError, DekuWriter};
use uuid::Uuid;

pub fn write_key<W: std::io::Write>(
    writer: &mut Writer<W>,
    value: &chacha20poly1305::Key,
) -> Result<(), DekuError> {
    let str_bytes = value.as_slice();
    str_bytes.to_writer(writer, ())
}

pub fn write_str<W: std::io::Write>(writer: &mut Writer<W>, value: &str) -> Result<(), DekuError> {
    let str_bytes = value.as_bytes();
    let str_len = str_bytes.len() as u32;
    str_len.to_writer(writer, ())?;
    str_bytes.to_writer(writer, ())
}

pub fn write_vec_str<W: std::io::Write>(
    writer: &mut Writer<W>,
    value: &Vec<String>,
) -> Result<(), DekuError> {
    let str_count = value.len() as u32;
    str_count.to_writer(writer, ())?;

    for str in value {
        write_str(writer, str)?;
    }

    Ok(())
}

pub fn write_uuid<W: std::io::Write>(
    writer: &mut Writer<W>,
    value: &Uuid,
) -> Result<(), DekuError> {
    let str = value.to_bytes_le();
    str.to_writer(writer, ())
}
