use binread::{BinRead, BinReaderExt, BinResult, NullString, ReadOptions};
use ssbh_lib::Ptr32;
use ssbh_write::SsbhWrite;
use std::io::{Cursor, Read, Seek, SeekFrom, Write};

#[derive(BinRead, Debug, SsbhWrite)]
pub struct Entry {
    pub name_offset: u32,
    pub property_count: u16,
    pub child_count: u16,
    pub property_start_index: i16,
    pub unk1: u16,
    pub parent_index: i16,
    pub unk2: i16, // always -1?
}

#[derive(BinRead, Debug, SsbhWrite)]
pub struct Property {
    pub name_offset: u32,
    pub value_offset: u32,
}

// TODO: This is probably an entityreference since XMB files with mapped entries define an id.
// TODO: Does this add a reference to value_offset to the entry at index unk_index?
// TODO: Add an idref attribute or create a new node?
#[derive(BinRead, Debug, SsbhWrite)]
pub struct MappedEntry {
    pub value_offset: u32,
    pub unk_index: u32, // parent entry?
}

// TODO: bigendian if node_count has FF000000 > 0?
#[derive(BinRead, Debug, SsbhWrite)]
#[br(magic = b"XMB ")]
pub struct Xmb {
    pub entry_count: u32,
    pub property_count: u32,
    pub string_count: u32,
    pub mapped_entry_count: u32,

    // TODO: Use this to cache the string lookups (names only)?
    // TODO: This seems to be all the names in the string buffer, not including values.
    #[br(count = string_count)]
    pub string_offsets: Ptr32<Vec<u32>>, // sorted in alphabetical order by string

    #[br(count = entry_count)]
    pub entries: Ptr32<Vec<Entry>>,

    #[br(count = property_count)]
    pub properties: Ptr32<Vec<Property>>,

    #[br(count = mapped_entry_count)]
    pub mapped_entries: Ptr32<Vec<MappedEntry>>,

    // This is technically two pointers to the string section.
    // Only the first pointer is read to avoid parsing/writing the data twice.
    pub string_data: Ptr32<StringBuffer>,
    pub string_value_offset: u32,

    // TODO: add align_after support per field for SsbhWrite
    padding1: u32,
    padding2: u128, // TODO: Is the header always padded to 64 bytes?
}

impl Xmb {
    pub fn read_name(&self, name_offset: u32) -> BinResult<String> {
        let mut reader = Cursor::new(&self.string_data.as_ref().unwrap().0);
        reader.seek(SeekFrom::Start(name_offset as u64))?;
        // TODO: Endianness doesn't matter for strings?
        let value: NullString = reader.read_le()?;
        Ok(value.to_string())
    }

    pub fn read_value(&self, value_offset: u32) -> BinResult<String> {
        let mut reader = Cursor::new(&self.string_data.as_ref().unwrap().0);
        // The offsets to the names and values are both relative to the start of the file.
        // Convert the values offset to a relative offset to use with the string buffer.
        let values_start_offset = self.string_value_offset as u64 - self.string_data.as_ref().unwrap().1;
        reader.seek(SeekFrom::Start(value_offset as u64 + values_start_offset))?;
        // TODO: Endianness doesn't matter for strings?
        let value: NullString = reader.read_le()?;
        Ok(value.to_string())
    }

    pub fn write<W: Write + Seek>(&self, writer: &mut W) -> std::io::Result<()> {
        // TODO: Magic support for SsbhWrite?
        writer.write_all(b"XMB ")?;
        let mut data_ptr = 4;
        self.ssbh_write(writer, &mut data_ptr)?;
        Ok(())
    }
}

// Store the buffer and its position.
// This is a workaround to use two absolute pointers to the string section with SsbhWrite.
#[derive(Debug)]
pub struct StringBuffer(Vec<u8>, u64);

impl BinRead for StringBuffer {
    type Args = ();

    fn read_options<R: Read + Seek>(
        reader: &mut R,
        _options: &ReadOptions,
        _args: Self::Args,
    ) -> BinResult<Self> {
        let pos = reader.stream_position()?;
        let mut buf = Vec::new();
        reader.read_to_end(&mut buf)?;
        Ok(Self(buf, pos))
    }
}

// TODO: SsbhWrite lacks a skip attribute for the saved position, so this can't be derived.
impl SsbhWrite for StringBuffer {
    fn ssbh_write<W: std::io::Write + std::io::Seek>(
        &self,
        writer: &mut W,
        data_ptr: &mut u64,
    ) -> std::io::Result<()> {
        let current_pos = writer.stream_position()?;
        if *data_ptr < current_pos + self.size_in_bytes() {
            *data_ptr = current_pos + self.size_in_bytes();
        }

        writer.write_all(&self.0)?;

        Ok(())
    }

    fn size_in_bytes(&self) -> u64 {
        self.0.len() as u64
    }

    fn alignment_in_bytes() -> u64 {
        4
    }
}
