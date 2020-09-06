use binread::{
    io::Read, io::Seek, io::SeekFrom, BinRead, BinReaderExt, BinResult, FilePtr, NullString,
    ReadOptions,
};
use serde::Serialize;
use std::collections::HashMap;
use std::fs::File;
use std::path::Path;

#[derive(BinRead, Debug)]
struct Entry {
    pub name_offset: u32,
    pub property_count: u16,
    pub child_count: u16,
    pub property_start_index: u16,
    pub unk1: u16,
    pub parent_index: i16,
    pub unk2: u16,
}

#[derive(BinRead, Debug)]
struct Property {
    pub name_offset: u32,
    pub value_offset: u32,
}

fn parse_offset_string_map<R: Read + Seek>(
    reader: &mut R,
    options: &ReadOptions,
    _: (),
) -> BinResult<HashMap<u32, String>> {
    let strings_ptr = u32::read_options(reader, options, ())?;
    let saved_pos = reader.seek(SeekFrom::Current(0))?;

    reader.seek(SeekFrom::Start(strings_ptr as u64))?;

    // There isn't a specified count for the number of strings,
    // so keep reading until finding an empty string.
    let mut string_by_offset = HashMap::new();
    loop {
        let current_pos = reader.seek(SeekFrom::Current(0))? as u32;
        let relative_offset = current_pos - strings_ptr;

        let text = NullString::read_options(reader, options, ())?;
        let text = text.into_string();
        if text == "" {
            break;
        }
        string_by_offset.insert(relative_offset, text);
    }

    reader.seek(SeekFrom::Start(saved_pos))?;

    Ok(string_by_offset)
}

// TODO: bigendian if node_count has FF000000 > 0?
#[derive(BinRead, Debug)]
#[br(magic = b"XMB ")]
struct Xmb {
    pub entry_count: u32,
    pub value_count: u32,
    pub property_count: u32,
    pub mapped_entries_count: u32,
    pub strings_ptr: FilePtr<u32, NullString>,

    #[br(count = entry_count)]
    pub entries: FilePtr<u32, Vec<Entry>>,

    #[br(count = property_count)]
    pub properties: FilePtr<u32, Vec<Property>>,

    pub node_map_ptr: u32,

    #[br(parse_with = parse_offset_string_map)]
    pub names: HashMap<u32, String>,

    #[br(parse_with = parse_offset_string_map)]
    pub values: HashMap<u32, String>,

    pub padding: u32,
}

#[derive(Debug, Serialize)]
pub struct XmbFileEntry {
    name: String,
    parent_index: i32,
    attributes: std::collections::HashMap<String, String>,
}

impl XmbFileEntry {
    fn new() -> XmbFileEntry {
        XmbFileEntry {
            name: "".to_string(),
            attributes: std::collections::HashMap::new(),
            parent_index: -1,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct XmbFile {
    entries: Vec<XmbFileEntry>,
}

impl XmbFile {
    fn new() -> XmbFile {
        let entries = Vec::new();
        XmbFile { entries }
    }
}

fn add_properties(file_entry: &mut XmbFileEntry, entry: &Entry, xmb_data: &Xmb) {
    for property_index in 0..entry.property_count {
        let property_index = (entry.property_start_index + property_index) as usize;
        let property = &xmb_data.properties[property_index];

        let key = xmb_data.names.get(&property.name_offset).unwrap();
        let value = xmb_data.values.get(&property.value_offset).unwrap();

        file_entry
            .attributes
            .insert(key.to_string(), value.to_string());
    }
}

fn create_file_entry(entry: &Entry, xmb_data: &Xmb) -> XmbFileEntry {
    let mut file_entry = XmbFileEntry::new();

    file_entry.name = xmb_data.names.get(&entry.name_offset).unwrap().to_string();
    file_entry.parent_index = entry.parent_index as i32;
    add_properties(&mut file_entry, entry, xmb_data);

    file_entry
}

fn create_xmb_file(xmb_data: Xmb) -> XmbFile {
    let mut xmb_file = XmbFile::new();
    for entry_index in 0..(xmb_data.entry_count as usize) {
        let entry = &xmb_data.entries[entry_index];
        let file_entry = create_file_entry(&entry, &xmb_data);
        xmb_file.entries.push(file_entry);
    }

    xmb_file
}

pub fn read_xmb(file: &Path) -> BinResult<XmbFile> {
    let mut f = File::open(&file)?;
    let xmb_data = f.read_le::<Xmb>()?;
    Ok(create_xmb_file(xmb_data))
}
