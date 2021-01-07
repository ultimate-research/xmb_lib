use binread::{io::Cursor, io::Read, io::Seek, io::SeekFrom, BinReaderExt, BinResult};
use serde::Serialize;
use std::fs;
use std::mem::size_of;
use std::path::Path;
use xmb::*;

mod xmb;

#[derive(Debug, Serialize)]
pub struct XmbFileEntry {
    pub name: String,
    pub parent_index: i32,
    pub attributes: std::collections::HashMap<String, String>,
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
    pub entries: Vec<XmbFileEntry>,
}

impl XmbFile {
    fn new() -> XmbFile {
        let entries = Vec::new();
        XmbFile { entries }
    }
}

fn add_properties<R: Read + Seek>(
    file_entry: &mut XmbFileEntry,
    entry: &Entry,
    xmb_data: &Xmb,
    reader: &mut R,
) {
    for property_index in 0..entry.property_count {
        let property_index = (entry.property_start_index + property_index) as usize;

        // TODO: error handling
        // There's no size for this array, so attempt to read
        // string offsets from the specified address.
        let property_offset =
            xmb_data.properties_ptr as usize + property_index * size_of::<Property>();
        reader
            .seek(SeekFrom::Start(property_offset as u64))
            .unwrap();
        let property: Property = reader.read_le().unwrap();

        let key = xmb_data.names.get(&property.name_offset).unwrap();
        let value = xmb_data.values.get(&property.value_offset).unwrap();

        file_entry
            .attributes
            .insert(key.to_string(), value.to_string());
    }
}

fn create_file_entry<R: Read + Seek>(
    entry: &Entry,
    xmb_data: &Xmb,
    reader: &mut R,
) -> XmbFileEntry {
    let mut file_entry = XmbFileEntry::new();

    file_entry.name = xmb_data.names.get(&entry.name_offset).unwrap().to_string();
    file_entry.parent_index = entry.parent_index as i32;
    add_properties(&mut file_entry, entry, xmb_data, reader);

    file_entry
}

fn create_xmb_file<R: Read + Seek>(xmb_data: Xmb, reader: &mut R) -> XmbFile {
    let mut xmb_file = XmbFile::new();
    for entry_index in 0..(xmb_data.entry_count as usize) {
        let entry = &xmb_data.entries[entry_index];
        let file_entry = create_file_entry(&entry, &xmb_data, reader);
        xmb_file.entries.push(file_entry);
    }

    xmb_file
}

pub fn read_xmb(file: &Path) -> BinResult<XmbFile> {
    // XMB files are small, so load the whole file into memory.
    let mut file = Cursor::new(fs::read(file)?);
    let xmb_data = file.read_le::<Xmb>()?;
    Ok(create_xmb_file(xmb_data, &mut file))
}
