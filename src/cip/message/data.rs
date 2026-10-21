use std::fmt::Debug;
use std::io::{Cursor, Read, Seek, Write};

use binrw::{BinRead, BinResult, BinWrite, Endian};

pub trait WriteSeekTrait: Write + Seek {}
impl<T: Write + Seek> WriteSeekTrait for T {}

// `Send + Sync` so packets carrying typed data can be held across `.await` points
pub trait CipData: Debug + Send + Sync {
    fn write_to(&self, w: &mut dyn WriteSeekTrait, endian: Endian) -> BinResult<()>;
}

impl<T> CipData for T
where
    T: for<'a> BinWrite<Args<'a> = ()> + Sized + Debug + Send + Sync,
{
    fn write_to(&self, mut w: &mut dyn WriteSeekTrait, endian: Endian) -> BinResult<()> {
        match endian {
            Endian::Little => self.write_le(&mut w),
            Endian::Big => self.write_be(&mut w),
        }
    }
}

#[derive(Debug)]
pub enum CipDataOpt {
    Raw(Vec<u8>),
    Typed(Box<dyn CipData>),
}

impl BinWrite for CipDataOpt {
    type Args<'a> = (u16,);

    fn write_options<W: Write + Seek>(
        &self,
        writer: &mut W,
        _endian: Endian,
        _args: Self::Args<'_>,
    ) -> BinResult<()> {
        match self {
            Self::Raw(data) => {
                writer.write_all(data)?;
            }
            Self::Typed(parsed) => {
                parsed.write_to(writer, _endian)?;
            }
        }
        Ok(())
    }
}

impl BinRead for CipDataOpt {
    type Args<'a> = (u16,);

    fn read_options<R: Read + Seek>(
        reader: &mut R,
        _endian: Endian,
        args: Self::Args<'_>,
    ) -> BinResult<Self> {
        // ALWAYS read a RequestDataOpt as a Raw Type
        let (len,) = args;
        let mut buf = vec![0u8; len as usize];
        reader.read_exact(&mut buf)?;
        Ok(Self::Raw(buf))
    }
}

impl PartialEq for CipDataOpt {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Raw(a), Self::Raw(b)) => a == b,
            (Self::Typed(a), Self::Typed(b)) => {
                // HACK: Compare the underlying binary data for equality

                // Compare by serializing both to bytes and checking equality
                let mut buf_a = Vec::new();
                let mut buf_b = Vec::new();
                let mut writer_a = Cursor::new(&mut buf_a);
                let mut writer_b = Cursor::new(&mut buf_b);

                let _ = a.write_to(&mut writer_a, Endian::Little);
                let _ = b.write_to(&mut writer_b, Endian::Little);

                buf_a == buf_b
            }
            (Self::Raw(raw), Self::Typed(parsed)) | (Self::Typed(parsed), Self::Raw(raw)) => {
                // PSEUDO_HACK: Compare the underlying binary data for equality
                let mut buf = Vec::new();
                let mut writer = Cursor::new(&mut buf);

                // Use write_options with correct arguments
                let _ = parsed.write_to(&mut writer, Endian::Little);

                buf == *raw
            }
        }
    }
}

impl Eq for CipDataOpt {}
