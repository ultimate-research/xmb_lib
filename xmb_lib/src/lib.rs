use binread::NullString;
use binread::{io::Cursor, BinReaderExt, BinResult};
use indexmap::{IndexMap, IndexSet};
use serde::Serialize;
use ssbh_lib::Ptr32;
use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::io::{Seek, Write};
use std::num::NonZeroU8;
use std::path::Path;
use xmb::*;
use xmltree::{Element, XMLNode};

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

#[derive(Debug, Clone)]
struct XmbEntryTemp {
    name: String,
    attributes: Vec<(String, String)>,
    parent_index: Option<usize>,
    child_count: usize,
    index: usize,
}

// Create temp types to flatten the list before writing offsets.
// This avoids leaving structs partially initialized with correct data.
// TODO: Is there a way to avoid this extra step?
fn add_temp_entries_recursive(
    children: &[XmbFileEntry],
    temp_entries: &mut Vec<XmbEntryTemp>,
    parent_index: Option<usize>,
) {
    let new_children: Vec<_> = children
        .iter()
        .enumerate()
        .map(|(i, child)| {
            let temp = XmbEntryTemp {
                name: child.name.clone(),
                attributes: child
                    .attributes
                    .iter()
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect(),
                parent_index,
                child_count: child.children.len(),
                index: temp_entries.len() + i,
            };

            temp
        })
        .collect();

    // Create a copy just as a way to know the parent index.
    // TODO: This is pretty inefficient.
    temp_entries.extend(new_children.clone());

    for (child, temp) in children.iter().zip(new_children) {
        add_temp_entries_recursive(&child.children, temp_entries, Some(temp.index));
    }
}

fn get_null_string(bytes: &[u8]) -> NullString {
    // TODO: This should take up to the first non zero byte rather than filtering 0 bytes.
    // TODO: This will probably be cleaner without using NullString.
    let bytes_nonzero: Vec<_> = bytes.iter().filter_map(|b| NonZeroU8::new(*b)).collect();
    bytes_nonzero.into()
}

// TODO: Find a way to test this conversion.
impl From<&XmbFile> for Xmb {
    fn from(xmb_file: &XmbFile) -> Self {
        // TODO: This could be more efficient by owning the XmbFile to avoid copying strings.

        // Flatten the tree by iterating in the expected entry order in the XMB file.
        let mut flattened_temp_entries = Vec::new();
        add_temp_entries_recursive(&xmb_file.entries, &mut flattened_temp_entries, None);

        // Calculate unk1 for each entry.
        for entry in &flattened_temp_entries {
            let unk1 = calculate_unk1(&flattened_temp_entries, entry);

            // println!("{} : -> {}", entry.index, unk1);
        }

        // TODO: Just calculate unk1 here?

        // 1. Collect the entry names and attribute names and sort alphabetically?
        // TODO: This can also initialize the offsets and string buffers.
        // TODO: Is this used for some sort of lookup?
        let mut names = IndexSet::new();
        let mut values = IndexSet::new();
        for entry in &flattened_temp_entries {
            names.insert(entry.name.clone());
            for (k, v) in &entry.attributes {
                names.insert(k.to_string());
                values.insert(v);
            }
        }

        // 2. Use these names to initialize the offsets.
        // It makes sense to make the buffers and offsets at the same time.
        // This avoids relying on string length.
        // TODO: Avoid unwrap.
        // TODO: How to sort the string_offsets alphabetically by string?
        let mut string_offsets = BTreeMap::new();
        let mut names_buffer = Cursor::new(Vec::new());
        let mut string_names = Vec::new();
        for name in names {
            let offset = names_buffer.stream_position().unwrap();
            string_offsets.insert(name.clone(), offset as u32);

            names_buffer.write_all(name.as_bytes()).unwrap();
            names_buffer.write_all(&[0u8]).unwrap();

            string_names.push((offset, get_null_string(name.as_bytes())));
        }

        let mut values_offsets = BTreeMap::new();
        let mut values_buffer = Cursor::new(Vec::new());
        let mut string_values = Vec::new();
        for value in values {
            let offset = values_buffer.stream_position().unwrap();
            values_offsets.insert(value.clone(), offset as u32);

            values_buffer.write_all(value.as_bytes()).unwrap();
            values_buffer.write_all(&[0u8]).unwrap();

            string_values.push((offset, get_null_string(value.as_bytes())));
        }

        // TODO: Add the value strings.
        // The string buffer seems to just use the order of appearance while only including unique values.
        // i.e. entry 1 name -> attribute 1 name ->  attribute 2 name -> entry 2 name -> ...
        // similar for attribute values

        let mut properties = Vec::new();

        let mut entries = Vec::new();
        for temp_entry in &flattened_temp_entries {
            // TODO: Rename properties to attributes?
            let property_start_index = if temp_entry.attributes.is_empty() {
                -1
            } else {
                properties.len() as i16
            };

            let unk1 = calculate_unk1(&flattened_temp_entries, temp_entry) as u16;

            let entry_properties: Vec<_> = temp_entry
                .attributes
                .iter()
                .map(|(k, v)| Property {
                    name_offset: *string_offsets.get(k).unwrap(),
                    value_offset: *values_offsets.get(v).unwrap(),
                })
                .collect();

            let entry = Entry {
                name_offset: *string_offsets.get(&temp_entry.name).unwrap(),
                property_count: entry_properties.len() as u16,
                child_count: temp_entry.child_count as u16,
                property_start_index,
                unk1,
                parent_index: temp_entry.parent_index.map(|i| i as i16).unwrap_or(-1),
                unk2: -1,
            };
            entries.push(entry);

            properties.extend(entry_properties);
        }

        // TODO: Mapped entries.
        let mapped_entries = Vec::new();

        Self {
            entry_count: entries.len() as u32,
            property_count: properties.len() as u32,
            string_count: string_offsets.len() as u32,
            mapped_entry_count: mapped_entries.len() as u32,
            string_offsets: Ptr32::new(string_offsets.values().map(|v| *v).collect()),
            entries: Ptr32::new(entries),
            properties: Ptr32::new(properties),
            mapped_entries: Ptr32::new(mapped_entries),
            string_names: Ptr32::new(StringBuffer(string_names)),
            string_values: Ptr32::new(StringBuffer(string_values)),
            padding1: 0,
            padding2: 0,
        }
    }
}

fn calculate_unk1(flattened_temp_entries: &Vec<XmbEntryTemp>, entry: &XmbEntryTemp) -> usize {
    // TODO: Create a function to find children?
    let child_indices: Vec<_> = flattened_temp_entries
        .iter()
        .filter(|c| c.parent_index == Some(entry.index))
        .map(|c| c.index)
        .collect();

    match child_indices.first() {
        Some(first_child) => *first_child,
        None => {
            // TODO: This case doesn't work for the last child of a node. 

            // Find the next sibling of the parent.
            // TODO: Avoid unwrap.
            let grandparent_index = flattened_temp_entries[entry.parent_index.unwrap()]
                .parent_index
                .unwrap();
            let parent_siblings: Vec<_> = flattened_temp_entries
                .iter()
                .filter(|c| c.parent_index == Some(grandparent_index))
                .collect();

            let parent_sibling_index = parent_siblings
                .iter()
                .position(|s| s.index == entry.parent_index.unwrap())
                .unwrap();
            let next_sibling = parent_siblings.get(parent_sibling_index + 1);
            // println!("{} {:?} {:?} {:?}", &entry.index, parent_siblings.iter().map(|c| c.index).collect::<Vec<_>>(), &next_sibling, parent_sibling_index);

            // If the next sibling has children, use the first child.
            // Otherwise, point past the end of the entries.
            // TODO: Is this just a recursive case of above?.
            match next_sibling {
                Some(next_sibling) => {

                    let child_indices: Vec<_> = flattened_temp_entries
                        .iter()
                        .filter(|c| c.parent_index == Some(next_sibling.index))
                        .map(|c| c.index)
                        .collect();

                    match child_indices.first() {
                        Some(first_child) => *first_child,
                        None => next_sibling.index,
                    }
                }
                None => flattened_temp_entries.len(),
            }
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

    // TODO: Test Xmb <-> XmbFile

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
