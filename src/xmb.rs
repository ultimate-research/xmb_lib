use binread::{
    io::Read, io::Seek, io::SeekFrom, BinRead, BinResult, FilePtr, NullString, ReadOptions,
};
use std::collections::HashMap;

#[derive(BinRead, Debug)]
pub struct Entry {
    pub name_offset: u32,
    pub property_count: u16,
    pub child_count: u16,
    pub property_start_index: u16,
    pub unk1: u16,
    pub parent_index: i16,
    pub unk2: u16,
}

#[derive(BinRead, Debug)]
pub struct Property {
    pub name_offset: u32,
    pub value_offset: u32,
}

// TODO: bigendian if node_count has FF000000 > 0?
#[derive(BinRead, Debug)]
#[br(magic = b"XMB ")]
pub struct Xmb {
    pub entry_count: u32,
    pub value_count: u32,
    pub property_count: u32,
    pub mapped_entries_count: u32,
    pub strings_ptr: FilePtr<u32, NullString>,

    #[br(count = entry_count)]
    pub entries: FilePtr<u32, Vec<Entry>>,

    pub properties_ptr: u32,
    pub node_map_ptr: u32,

    // The names and values are 32 bit pointers to string arrays with unspecified length.
    // Store them as hashmaps to convert relative offsets in the arrays to strings.
    #[br(parse_with = parse_offset_string_map)]
    pub names: HashMap<u32, String>,

    #[br(parse_with = parse_offset_string_map)]
    pub values: HashMap<u32, String>,

    pub padding: u32,
}

fn parse_offset_string_map<R: Read + Seek>(
    reader: &mut R,
    options: &ReadOptions,
    _: (),
) -> BinResult<HashMap<u32, String>> {
    let strings_ptr = u32::read_options(reader, options, ())?;
    let saved_pos = reader.seek(SeekFrom::Current(0))?;

    reader.seek(SeekFrom::Start(strings_ptr as u64))?;

    let mut string_by_offset = HashMap::new();
    loop {
        let current_pos = reader.seek(SeekFrom::Current(0))? as u32;
        let relative_offset = current_pos - strings_ptr;

        // There isn't a specified count for the strings table, so keep trying to read.
        match NullString::read_options(reader, options, ()) {
            Ok(text) => {
                string_by_offset.insert(relative_offset, text.into_string());
            }
            Err(_) => break,
        }
    }

    reader.seek(SeekFrom::Start(saved_pos))?;

    Ok(string_by_offset)
}
