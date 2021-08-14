use binread::{BinRead, BinReaderExt, BinResult, FilePtr, NullString, ReadOptions};
use std::io::{Cursor, Read, Seek, SeekFrom};

#[derive(BinRead, Debug)]
pub struct Entry {
    pub name_offset: u32,
    pub property_count: u16,
    pub child_count: u16,
    pub property_start_index: i16,
    pub unk1: u16,
    pub parent_index: i16,
    pub unk2: i16,
}

#[derive(BinRead, Debug)]
pub struct Property {
    pub name_offset: u32,
    pub value_offset: u32,
}

#[derive(BinRead, Debug)]
pub struct MappedEntry {
    pub value_offset: u32,
    pub unk_index: u32, // parent entry?
}

// TODO: bigendian if node_count has FF000000 > 0?
#[derive(BinRead, Debug)]
#[br(magic = b"XMB ")]
pub struct Xmb {
    pub entry_count: u32,
    pub property_count: u32,
    pub string_count: u32,
    pub mapped_entry_count: u32,

    #[br(count = string_count)] // is this the correct count?
    pub string_offsets: FilePtr<u32, Vec<u32>>, // sorted in alphabetical order by string

    #[br(count = entry_count)]
    pub entries: FilePtr<u32, Vec<Entry>>,

    #[br(count = property_count)]
    pub properties: FilePtr<u32, Vec<Property>>,

    #[br(count = mapped_entry_count)]
    pub mapped_entries: FilePtr<u32, Vec<MappedEntry>>,

    pub string_data: FilePtr<u32, StringBuffer>,
    pub string_values_offset: u32,
    // TODO: Is the header always padded to 64 bytes?
}

impl Xmb {
    pub fn read_name(&self, name_offset: u32) -> BinResult<String> {
        let mut reader = Cursor::new(&self.string_data.0);
        reader.seek(SeekFrom::Start(name_offset as u64))?;
        // TODO: Endianness doesn't matter for strings?
        let value: NullString = reader.read_le()?;
        Ok(value.to_string())
    }

    pub fn read_value(&self, value_offset: u32) -> BinResult<String> {
        let mut reader = Cursor::new(&self.string_data.0);

        // Account for the values offset being relative to the start of the file and not the buffer.
        // TODO: There's probably a cleaner/safer way to write this.
        reader.seek(SeekFrom::Start(
            self.string_values_offset as u64 + value_offset as u64 - self.string_data.ptr as u64,
        ))?;
        // TODO: Endianness doesn't matter for strings?
        let value: NullString = reader.read_le()?;
        Ok(value.to_string())
    }
}

#[derive(BinRead, Debug)]
pub struct StringBuffer(#[br(parse_with = read_to_end)] Vec<u8>);

fn read_to_end<R: Read + Seek>(reader: &mut R, _ro: &ReadOptions, _: ()) -> BinResult<Vec<u8>> {
    let mut buf = Vec::new();
    reader.read_to_end(&mut buf)?;
    Ok(buf)
}
