use binread::{io::Cursor, BinReaderExt, BinResult};
use serde::Serialize;
use std::collections::HashMap;
use std::fs;

use std::path::Path;
use xmb::*;

mod xmb;

#[derive(Debug, Serialize)]
pub struct XmbFileEntry {
    pub name: String,
    pub parent_index: i32,
    pub attributes: std::collections::HashMap<String, String>,
}

#[derive(Debug, Serialize)]
pub struct XmbFile {
    pub entries: Vec<XmbFileEntry>,
}

fn get_attributes(xmb_data: &Xmb, entry: &Entry) -> HashMap<String, String> {
    (0..entry.property_count)
        .map(|i| {
            // TODO: Don't perform unchecked arithmetic and indexing with signed numbers.
            // TODO: Start index doesn't seem to work for effect_locator.xmb files?
            let property_index = (entry.property_start_index as u16 + i) as usize;
            let property = &xmb_data.properties[property_index];
            let key = xmb_data.read_name(property.name_offset).unwrap();
            let value = xmb_data.read_value(property.value_offset).unwrap();
            (key, value)
        })
        .collect()
}

fn create_file_entry(xmb_data: &Xmb, entry: &Entry) -> XmbFileEntry {
    XmbFileEntry {
        name: xmb_data.read_name(entry.name_offset).unwrap(),
        parent_index: entry.parent_index as i32,
        attributes: get_attributes(xmb_data, entry),
    }
}

fn create_xmb_file(xmb_data: Xmb) -> XmbFile {
    XmbFile {
        entries: xmb_data
            .entries
            .iter()
            .map(|e| create_file_entry(&xmb_data, &e))
            .collect(),
    }
}

pub fn read_xmb(file: &Path) -> BinResult<XmbFile> {
    // XMB files are small, so load the whole file into memory.
    let mut file = Cursor::new(fs::read(file)?);
    let xmb_data = file.read_le::<Xmb>()?;
    // println!("{:#?}", xmb_data);

    Ok(create_xmb_file(xmb_data))
}
