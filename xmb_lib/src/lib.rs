use binread::{io::Cursor, BinReaderExt, BinResult};
use indexmap::IndexMap;
use serde::Serialize;
use ssbh_lib::Ptr32;
use std::collections::HashMap;
use std::fs;
use xmltree::{Element, XMLNode};

use std::path::Path;
use xmb::*;

pub mod xmb;

// TODO: Deserialize?
#[derive(Debug, Serialize)]
pub struct Attributes(HashMap<String, String>);

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct XmbFileEntry {
    pub name: String,
    pub attributes: IndexMap<String, String>,
    pub children: Vec<XmbFileEntry>,
    pub mapped_children: Vec<XmbFileEntry>,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct XmbFile {
    pub entries: Vec<XmbFileEntry>,
}

impl XmbFile {
    pub fn to_xml(&self) -> Element {
        // TODO: Don't assume this is the root entry or that there is a single root?
        let entry = &self.entries[0];
        create_element_recursive(self, entry)
    }

    pub fn from_xml(root: &Element) -> Self {
        // TODO: Multiple root nodes?
        let root = create_entry_from_xml_recursive(root);
        Self {
            entries: vec![root],
        }
    }
}

// TODO: All these conversions can use test cases.
fn create_entry_from_xml_recursive(xml_node: &Element) -> XmbFileEntry {
    let children = xml_node
        .children
        .iter()
        .filter_map(XMLNode::as_element)
        .map(create_entry_from_xml_recursive)
        .collect();

    XmbFileEntry {
        name: xml_node.name.clone(),
        attributes: xml_node.attributes.clone(),
        children,
        // TODO: How to handle mapped entries?
        mapped_children: Vec::new(),
    }
}

// TODO: From<XmbFile> for Xmb
impl From<&Xmb> for XmbFile {
    fn from(xmb: &Xmb) -> Self {
        xmb_file_from_xmb(xmb)
    }
}

impl From<&XmbFile> for Xmb {
    fn from(xmb_file: &XmbFile) -> Self {
        // TODO: Go in BFS order to "flatten" the entries?

        // TODO: The offsets aren't yet known.
        // TODO: Just collect the parent indices as a temporary step?
        let mut entries = Vec::new();
        for xmb_file_entry in &xmb_file.entries {
            let entry = Entry {
                name_offset: 0,
                property_count: 0,
                child_count: 0,
                property_start_index: 0,
                unk1: 0,
                parent_index: 0,
                unk2: 0,
            };
            entries.push(entry);
        }

        let properties = Vec::new();
        let mapped_entries = Vec::new();
        let string_offsets = Vec::new();

        // TODO: Properly initialize these fields.
        Self {
            entry_count: entries.len() as u32,
            property_count: properties.len() as u32,
            string_count: 0,
            mapped_entry_count: mapped_entries.len() as u32,
            string_offsets: Ptr32::new(string_offsets),
            entries: Ptr32::new(entries),
            properties: Ptr32::new(properties),
            mapped_entries: Ptr32::new(mapped_entries),
            string_data: Ptr32::new(StringBuffer(Vec::new(), 0)),
            string_value_offset: 0,
            padding1: 0,
            padding2: 0,
        }
    }
}

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
        mapped_children,
    }
}

fn xmb_file_from_xmb(xmb_data: &Xmb) -> XmbFile {
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

    Ok(xmb_file_from_xmb(&xmb_data))
}

// TODO: Separate file for XmbFile types?
#[cfg(test)]
mod tests {
    // XMB is a binary version of XML, so construct XML documents by hand.
    // This tests the necessary format features with substantially smaller test cases.
    use super::*;
    use indexmap::indexmap;

    #[test]
    fn xmb_file_to_from_xml() {
        // TODO: Indoc?
        let data = r#"
        <?xml version="1.0" encoding="UTF-8"?>
        <root a="1" b="2">
            <child1 a="3" b="4">
                <subchild1 c="7" d="8" e="f"/> 
            </child1>
            <child2 a="5" b="6"/>
        </root>"#;
        let element = Element::parse(data.as_bytes()).unwrap();

        let xmb_file = XmbFile::from_xml(&element);
        assert_eq!(
            XmbFile {
                entries: vec![XmbFileEntry {
                    name: "root".into(),
                    attributes: indexmap!["a".into() => "1".into(), "b".into() => "2".into()],
                    children: vec![
                        XmbFileEntry {
                            name: "child1".into(),
                            attributes: indexmap!["a".into() => "3".into(), "b".into() => "4".into()],
                            children: vec![XmbFileEntry {
                                name: "subchild1".into(),
                                attributes: indexmap![
                                    "c".into() => "7".into(),
                                    "d".into() => "8".into(),
                                    "e".into() => "f".into()
                                ],
                                children: Vec::new(),
                                mapped_children: Vec::new()
                            }],
                            mapped_children: Vec::new()
                        },
                        XmbFileEntry {
                            name: "child2".into(),
                            attributes: indexmap!["a".into() => "5".into(), "b".into() => "6".into()],
                            children: Vec::new(),
                            mapped_children: Vec::new()
                        }
                    ],
                    mapped_children: Vec::new()
                }]
            },
            xmb_file
        );

        // Just test the tree representation to avoid testing formatting differences.
        let output_element = xmb_file.to_xml();
        assert_eq!(element, output_element);
    }

    // TODO: Test mapped entries?
}
