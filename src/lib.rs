use binread::{io::Cursor, BinReaderExt, BinResult};
use indexmap::IndexMap;
use serde::Serialize;
use std::collections::HashMap;
use std::fs;
use xmltree::{Element, XMLNode};

use std::path::Path;
use xmb::*;

pub mod xmb;

// TODO: Deserialize?
#[derive(Debug, Serialize)]
pub struct Attributes(HashMap<String, String>);

#[derive(Debug, Serialize)]
pub struct XmbFileEntry {
    pub name: String,
    pub attributes: IndexMap<String, String>,
    pub children: Vec<XmbFileEntry>,
    pub mapped_children: Vec<XmbFileEntry>,
}

#[derive(Debug, Serialize)]
pub struct XmbFile {
    pub entries: Vec<XmbFileEntry>,
}

impl XmbFile {
    pub fn to_xml(&self) -> Element {
        // TODO: Don't assume this is the root entry or that there is a single root?
        let entry = &self.entries[0];
        create_element_recursive(self, entry)
    }
}

// TODO: From<XmbFile> for Xmb
// TODO: from_xml for XmbFile

fn create_element_recursive(xmb: &XmbFile, entry: &XmbFileEntry) -> Element {
    // Just create child elements for each mapped entry for now.
    let children: Vec<_> = entry
        .children
        .iter()
        .chain(entry.mapped_children.iter())
        .map(|e| XMLNode::Element(create_element_recursive(xmb, e)))
        .collect();

    xmltree::Element {
        prefix: None,
        namespace: None,
        namespaces: None,
        name: entry.name.clone(),
        attributes: entry.attributes.clone(),
        children,
    }
}

impl From<&Xmb> for XmbFile {
    fn from(xmb: &Xmb) -> Self {
        create_xmb_file(xmb)
    }
}

fn get_attributes(xmb_data: &Xmb, entry: &Entry) -> IndexMap<String, String> {
    (0..entry.property_count)
        .map(|i| {
            // TODO: Don't perform unchecked arithmetic and indexing with signed numbers.
            // TODO: Start index doesn't seem to work for effect_locator.xmb files?
            let property_index = (entry.property_start_index as u16 + i) as usize;
            let property = &xmb_data.properties.as_ref().unwrap()[property_index];
            let key = xmb_data.read_name(property.name_offset).unwrap();
            let value = xmb_data.read_value(property.value_offset).unwrap();
            (key, value)
        })
        .collect()
}

// TODO: Try to find a more straightforward iterative approach.
// It should be doable to iterate the entry list at most twice?
fn create_children_recursive(xmb_data: &Xmb, entry: &Entry, entry_index: i16) -> XmbFileEntry {
    let child_entries: Vec<_> = xmb_data
        .entries
        .as_ref()
        .unwrap()
        .iter()
        .enumerate()
        .filter(|(_, e)| e.parent_index == entry_index)
        .collect();

    let children: Vec<_> = child_entries
        .iter()
        .map(|(i, e)| create_children_recursive(xmb_data, e, *i as i16))
        .collect();

    let mapped_children: Vec<_> = xmb_data
        .mapped_entries
        .as_ref()
        .unwrap()
        .iter()
        .filter(|e| e.unk_index as i16 == entry_index)
        .map(|e| {
            let mut attributes = IndexMap::new();
            attributes.insert(
                "id".to_string(),
                xmb_data.read_value(e.value_offset).unwrap(),
            );

            XmbFileEntry {
                name: "mapped_entry".to_string(),
                attributes,
                children: Vec::new(),
                mapped_children: Vec::new(),

            }
        })
        .collect();

    XmbFileEntry {
        name: xmb_data.read_name(entry.name_offset).unwrap(),
        attributes: get_attributes(xmb_data, entry),
        children,
        mapped_children
    }
}

fn create_xmb_file(xmb_data: &Xmb) -> XmbFile {
    // First find the nodes with no parents.
    // Then recursively add their children based on the parent index.
    let roots: Vec<_> = xmb_data
        .entries
        .as_ref()
        .unwrap()
        .iter()
        .enumerate()
        .filter(|(_, e)| e.parent_index == -1)
        .map(|(i, e)| create_children_recursive(&xmb_data, e, i as i16))
        .collect();

    XmbFile { entries: roots }
}

// TODO: Support a user specified reader or writer.
pub fn read_xmb(file: &Path) -> BinResult<XmbFile> {
    // XMB files are small, so load the whole file into memory.
    let mut file = Cursor::new(fs::read(file)?);
    let xmb_data = file.read_le::<Xmb>()?;

    Ok(create_xmb_file(&xmb_data))
}
