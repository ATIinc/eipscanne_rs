use binrw::{BinRead, BinResult, BinWrite, Endian};
use std::io::{Read, Seek, Write};
#[cfg(feature = "async")]
use std::marker::{Send, Sync};

pub trait WriteSeekTrait: Write + Seek {}
impl<T: Write + Seek> WriteSeekTrait for T {}

// This conditionally includes a module which implements WEBP support.
#[cfg(feature = "async")]
pub trait CipData: std::fmt::Debug + Send + Sync {
    fn write_to(&self, w: &mut dyn WriteSeekTrait, endian: Endian) -> BinResult<()>;
}

#[cfg(feature = "async")]
impl<T> CipData for T
where
    T: for<'a> BinRead<Args<'a> = ()> + for<'a> BinWrite<Args<'a> = ()> + Sized + std::fmt::Debug + Send + Sync,
{
    fn write_to(&self, mut w: &mut dyn WriteSeekTrait, endian: Endian) -> BinResult<()> {
        match endian {
            Endian::Little => self.write_le(&mut w),
            Endian::Big => self.write_be(&mut w),
        }
    }
}

#[cfg(not(feature = "async"))]
pub trait CipData: std::fmt::Debug {
    fn write_to(&self, w: &mut dyn WriteSeekTrait, endian: Endian) -> BinResult<()>;
}

#[cfg(not(feature = "async"))]
impl<T> CipData for T
where
    T: for<'a> BinRead<Args<'a> = ()> + for<'a> BinWrite<Args<'a> = ()> + Sized + std::fmt::Debug,
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
        _endian: binrw::Endian,
        _args: Self::Args<'_>,
    ) -> BinResult<()> {
        use CipDataOpt::*;
        match self {
            Raw(data) => {
                writer.write_all(data)?;
            }
            Typed(parsed) => {
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
        _endian: binrw::Endian,
        args: Self::Args<'_>,
    ) -> BinResult<Self> {
        // ALWAYS read a RequestDataOpt as a Raw Type
        let (len,) = args;
        let mut buf = vec![0u8; len as usize];
        reader.read_exact(&mut buf)?;
        Ok(CipDataOpt::Raw(buf))
    }
}

impl PartialEq for CipDataOpt {
    fn eq(&self, other: &Self) -> bool {
        use CipDataOpt::*;
        match (self, other) {
            (Raw(a), Raw(b)) => a == b,
            (Typed(a), Typed(b)) => {
                // HACK: Compare the underlying binary data for equality

                // Compare by serializing both to bytes and checking equality
                let mut buf_a = Vec::new();
                let mut buf_b = Vec::new();
                let mut writer_a = std::io::Cursor::new(&mut buf_a);
                let mut writer_b = std::io::Cursor::new(&mut buf_b);

                let _ = a.write_to(&mut writer_a, binrw::Endian::Little);
                let _ = b.write_to(&mut writer_b, binrw::Endian::Little);

                buf_a == buf_b
            }
            (Raw(raw), Typed(parsed)) | (Typed(parsed), Raw(raw)) => {
                // PSEUDO_HACK: Compare the underlying binary data for equality
                let mut buf = Vec::new();
                let mut writer = std::io::Cursor::new(&mut buf);

                // Use write_options with correct arguments
                let _ = parsed.write_to(&mut writer, binrw::Endian::Little);

                buf == *raw
            }
        }
    }
}

impl Eq for CipDataOpt {}
