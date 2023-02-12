use binrw::io::Cursor;
use indexmap::{IndexMap, IndexSet};
use ssbh_lib::Ptr32;
use std::collections::BTreeMap;
use std::convert::{TryFrom, TryInto};
use std::error::Error;
use std::io::{Read, Seek, Write};
use std::path::Path;
use thiserror::Error;
use xmb::*;
use xmltree::{Element, XMLNode};

pub mod xmb;

#[derive(Debug, Error)]
pub enum CreateXmlError {
    // TODO: Are empty XMB documents valid?
    #[error("failed to find a root element")]
    MissingRootElement,
    #[error("encountered more than one root element")]
    MultipleRootElements,
}

#[derive(Debug, Error)]
pub enum ReadXmbError {
    // TODO: Create meaningful error variants.
    #[error("encountered a null pointer")]
    NullError,

    #[error("failed to parse XMB binary")]
    BinRead(#[from] binrw::Error),

    #[error("failed to parse XMB binary")]
    Io(#[from] std::io::Error),
}

// TODO: Deserialize?
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[derive(Debug, PartialEq, Eq)]
pub struct XmbFileEntry {
    pub name: String,
    pub attributes: IndexMap<String, String>,
    pub children: Vec<XmbFileEntry>,
}

#[cfg(feature = "arbitrary")]
impl<'a> arbitrary::Arbitrary<'a> for XmbFileEntry {
    fn arbitrary(u: &mut arbitrary::Unstructured<'a>) -> arbitrary::Result<Self> {
        Ok(Self {
            name: u.arbitrary()?,
            attributes: IndexMap::from_iter(u.arbitrary::<Vec<(String, String)>>()?),
            children: u.arbitrary()?,
        })
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, PartialEq, Eq)]
pub struct XmbFile {
    pub entries: Vec<XmbFileEntry>,
}

impl XmbFile {
    pub fn to_xml(&self) -> Result<Element, CreateXmlError> {
        let mut root_entries = self.entries.iter();
        let entry = root_entries
            .next()
            .ok_or(CreateXmlError::MissingRootElement)?;

        if root_entries.next().is_some() {
            return Err(CreateXmlError::MultipleRootElements);
        }

        Ok(create_element_recursive(self, entry))
    }

    pub fn from_xml(root: &Element) -> Self {
        // TODO: Multiple root nodes?
        let root = create_entry_from_xml_recursive(root);
        Self {
            entries: vec![root],
        }
    }

    pub fn read<R: Read + Seek>(reader: &mut R) -> Result<Self, Box<dyn Error>> {
        Xmb::read(reader)?.try_into().map_err(Into::into)
    }

    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self, Box<dyn Error>> {
        Xmb::from_file(path)?.try_into().map_err(Into::into)
    }

    pub fn write<W: Write + Seek>(&self, writer: &mut W) -> std::io::Result<()> {
        Xmb::from(self).write(writer)
    }

    pub fn write_to_file<P: AsRef<Path>>(&self, path: P) -> std::io::Result<()> {
        Xmb::from(self).write_to_file(path)
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
    }
}

impl TryFrom<Xmb> for XmbFile {
    type Error = ReadXmbError;

    fn try_from(xmb: Xmb) -> Result<Self, Self::Error> {
        Self::try_from(&xmb)
    }
}

// TODO: Make a separate XmbError without IO errors?
impl TryFrom<&Xmb> for XmbFile {
    type Error = ReadXmbError;

    fn try_from(xmb: &Xmb) -> Result<Self, Self::Error> {
        create_xmb_file(xmb).ok_or(ReadXmbError::NullError)
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

// TODO: Is this just BFS order?
// Create temp types to flatten the list before writing offsets.
// This avoids leaving structs partially initialized.
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
            // Rust strings allow null bytes but XMB does not.
            // For now, just strip nulls.
            XmbEntryTemp {
                name: child.name.replace("\0", ""),
                attributes: child
                    .attributes
                    .iter()
                    .map(|(k, v)| (k.replace("\0", ""), v.replace("\0", "")))
                    .collect(),
                parent_index,
                child_count: child.children.len(),
                index: temp_entries.len() + i,
            }
        })
        .collect();

    // Create a copy just as a way to know the parent index.
    // TODO: This is pretty inefficient.
    temp_entries.extend(new_children.clone());

    for (child, temp) in children.iter().zip(new_children) {
        add_temp_entries_recursive(&child.children, temp_entries, Some(temp.index));
    }
}

// TODO: Find a way to test this conversion.
// TODO: This should be try_from or it's own method.
impl From<&XmbFile> for Xmb {
    fn from(xmb_file: &XmbFile) -> Self {
        // TODO: This could be more efficient by owning the XmbFile to avoid copying strings.

        // Flatten the tree by iterating in the expected entry order in the XMB file.
        let mut flattened_temp_entries = Vec::new();
        add_temp_entries_recursive(&xmb_file.entries, &mut flattened_temp_entries, None);

        // Collect unique names and values as they appear in the flattened entries.
        // TODO: This can also initialize the offsets and string buffers.
        // TODO: Is this used for some sort of lookup?
        let mut names = IndexSet::new();
        let mut values = IndexSet::new();
        for entry in &flattened_temp_entries {
            names.insert(entry.name.clone());
            for (k, v) in &entry.attributes {
                names.insert(k.clone());
                values.insert(v);
            }
        }

        // Use these names to initialize the offsets.
        // It makes sense to make the buffers and offsets at the same time.
        // This avoids relying on string length.
        // TODO: Avoid unwrap.
        let mut string_offsets = BTreeMap::new();
        let mut names_buffer = Cursor::new(Vec::new());
        for name in names {
            let offset = names_buffer.stream_position().unwrap();
            string_offsets.insert(name.clone(), offset as u32);

            // TODO: This writes additional null bytes for empty strings?
            // TODO: Empty strings won't have a null byte?
            // TODO: This will only read one null byte when reading a new xmb even if we write more?
            names_buffer.write_all(name.as_bytes()).unwrap();
            names_buffer.write_all(&[0u8]).unwrap();
        }

        let mut values_offsets = BTreeMap::new();
        let mut values_buffer = Cursor::new(Vec::new());
        for value in values {
            let offset = values_buffer.stream_position().unwrap();
            values_offsets.insert(value.clone(), offset as u32);

            values_buffer.write_all(value.as_bytes()).unwrap();
            values_buffer.write_all(&[0u8]).unwrap();
        }

        // Collect all entries and attributes.
        let mut attributes = Vec::new();

        // Collect strings for id attributes and corresponding node indices.
        // The lookup is sorted alphabetically by the "id" value.
        let mut entry_index_by_id = BTreeMap::new();
        for (i, temp_entry) in flattened_temp_entries.iter().enumerate() {
            // Assume only the "id" attribute is used for lookups.
            // This seems to be the case for Smash Ultimate and Smash 4.
            let id_value = temp_entry
                .attributes
                .iter()
                .find(|(k, _v)| k == "id")
                .map(|(_k, v)| v);

            if let Some(id_value) = id_value {
                entry_index_by_id.insert(id_value, i);
            }
        }

        let mapped_entries: Vec<_> = entry_index_by_id
            .iter()
            .map(|(id_value, entry_index)| MappedEntry {
                value_offset: *values_offsets.get(*id_value).unwrap(),
                entry_index: *entry_index as u32,
            })
            .collect();

        let mut entries = Vec::new();
        for temp_entry in &flattened_temp_entries {
            let attribute_start_index = if temp_entry.attributes.is_empty() {
                -1
            } else {
                attributes.len() as i16
            };

            let unk1 = calculate_unk1(temp_entry, &flattened_temp_entries) as i16;

            let entry_attributes: Vec<_> = temp_entry
                .attributes
                .iter()
                .map(|(k, v)| Attribute {
                    name_offset: *string_offsets.get(k).unwrap(),
                    value_offset: *values_offsets.get(v).unwrap(),
                })
                .collect();

            let entry = Entry {
                name_offset: *string_offsets.get(&temp_entry.name).unwrap(),
                attribute_count: entry_attributes.len() as u16,
                child_count: temp_entry.child_count as u16,
                attribute_start_index,
                unk1,
                parent_index: temp_entry.parent_index.map(|i| i as i16).unwrap_or(-1),
                unk2: -1,
            };
            entries.push(entry);

            attributes.extend(entry_attributes);
        }

        Self {
            entry_count: entries.len() as u32,
            attribute_count: attributes.len() as u32,
            string_count: string_offsets.len() as u32,
            mapped_entry_count: mapped_entries.len() as u32,
            string_offsets: XmbVec::new(string_offsets.values().copied().collect()),
            entries: XmbVec::new(entries),
            attributes: XmbVec::new(attributes),
            mapped_entries: XmbVec::new(mapped_entries),
            string_names: Ptr32::new(NamesBuffer(names_buffer.into_inner())),
            string_values: Ptr32::new(ValuesBuffer(values_buffer.into_inner())),
        }
    }
}

// TODO: This is probably not the simplest or most efficient way to write this.
fn calculate_unk1(entry: &XmbEntryTemp, flattened_temp_entries: &[XmbEntryTemp]) -> usize {
    // TODO: Create a function to find children?
    let child_indices: Vec<_> = flattened_temp_entries
        .iter()
        .filter(|c| c.parent_index == Some(entry.index))
        .map(|c| c.index)
        .collect();

    match child_indices.first() {
        Some(first_child) => *first_child,
        None => {
            let parent = find_parent(entry, flattened_temp_entries);
            calculate_unk1_leaf(entry, parent, flattened_temp_entries)
                .unwrap_or(flattened_temp_entries.len())
        }
    }
}

fn calculate_unk1_leaf(
    entry: &XmbEntryTemp,
    parent: Option<&XmbEntryTemp>,
    flattened_temp_entries: &[XmbEntryTemp],
) -> Option<usize> {
    // Cover the base case by returning None if there is no parent.
    // For the rightmost node at the leaf level, this will traverse up the tree.
    // The root node has no parent and will return None.
    let parent = parent?;

    // TODO: It might be simpler to just match on find_next_sibling for parent.
    match find_parent(parent, flattened_temp_entries) {
        Some(grand_parent) => {
            let next_sibling = find_next_sibling(parent, flattened_temp_entries);

            // TODO: There's a case for some lod.xmb files where this can return -1?

            // Use the first child of the parent's next sibling.
            // If this doesn't work, recurse up the tree.
            match next_sibling {
                Some(next_sibling) => {
                    // TODO: Reuse this find children function?
                    let child_indices: Vec<_> = flattened_temp_entries
                        .iter()
                        .filter(|c| c.parent_index == Some(next_sibling.index))
                        .map(|c| c.index)
                        .collect();

                    match child_indices.first() {
                        Some(first_child) => Some(*first_child),
                        None => {
                            calculate_unk1_leaf(entry, Some(grand_parent), flattened_temp_entries)
                        }
                    }
                }
                None => calculate_unk1_leaf(entry, Some(grand_parent), flattened_temp_entries),
            }
        }
        None => {
            // It's possible for only some of a node's children to be leaves (no children).
            // This case comes up in some model.xmb files.
            let next_sibling = find_next_sibling_with_children(entry, flattened_temp_entries);
            next_sibling.map(|s| s.index)
        }
    }
}

// TODO: These can be methods on XmbEntryTemp.
fn find_parent<'a>(
    entry: &'a XmbEntryTemp,
    flattened_temp_entries: &'a [XmbEntryTemp],
) -> Option<&'a XmbEntryTemp> {
    entry
        .parent_index
        .and_then(|i| flattened_temp_entries.get(i))
}

fn find_next_sibling<'a>(
    entry: &'a XmbEntryTemp,
    flattened_temp_entries: &'a [XmbEntryTemp],
) -> Option<&'a XmbEntryTemp> {
    let parent_index = entry.parent_index?;

    let siblings: Vec<_> = flattened_temp_entries
        .iter()
        .filter(|c| c.parent_index == Some(parent_index))
        .collect();

    let sibling_index = siblings
        .iter()
        .position(|s| s.index == entry.index)
        .unwrap();

    siblings.get(sibling_index + 1).copied()
}

fn find_next_sibling_with_children<'a>(
    entry: &'a XmbEntryTemp,
    flattened_temp_entries: &'a [XmbEntryTemp],
) -> Option<&'a XmbEntryTemp> {
    let parent_index = entry.parent_index?;
    let siblings: Vec<_> = flattened_temp_entries
        .iter()
        .filter(|c| c.parent_index == Some(parent_index))
        .collect();

    let sibling_index = siblings
        .iter()
        .position(|s| s.index == entry.index)
        .unwrap();

    // Find the first child of the next sibling after the current entry with children.
    siblings
        .iter()
        .skip(sibling_index + 1)
        .filter_map(|c| {
            let first_child = flattened_temp_entries
                .iter()
                .find(|c1| c1.parent_index == Some(c.index));
            first_child
        })
        .next()
}

fn create_element_recursive(xmb: &XmbFile, entry: &XmbFileEntry) -> Element {
    // Just create child elements for each mapped entry for now.
    let children: Vec<_> = entry
        .children
        .iter()
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

fn get_attributes(xmb_data: &Xmb, entry: &Entry) -> Option<IndexMap<String, String>> {
    (0..entry.attribute_count)
        .map(|i| {
            // TODO: Don't perform unchecked arithmetic and indexing with signed numbers.
            let attribute_index = (entry.attribute_start_index as u16 + i) as usize;
            let attribute = &xmb_data.attributes.0.as_ref()?.get(attribute_index)?;
            let key = xmb_data.read_name(attribute.name_offset)?;
            let value = xmb_data.read_value(attribute.value_offset)?;
            Some((key, value))
        })
        .collect()
}

// TODO: Try to find a more straightforward iterative approach.
// It should be doable to iterate the entry list at most twice?
fn create_children_recursive(
    xmb_data: &Xmb,
    entry: &Entry,
    entry_index: i16,
) -> Option<XmbFileEntry> {
    let child_entries: Vec<_> = xmb_data
        .entries
        .0
        .as_ref()
        .unwrap()
        .iter()
        .enumerate()
        .filter(|(_, e)| e.parent_index == entry_index)
        .collect();

    let children: Vec<_> = child_entries
        .iter()
        .map(|(i, e)| create_children_recursive(xmb_data, e, *i as i16))
        .collect::<Option<Vec<_>>>()?;

    Some(XmbFileEntry {
        name: xmb_data.read_name(entry.name_offset)?,
        attributes: get_attributes(xmb_data, entry)?,
        children,
    })
}

fn create_xmb_file(xmb_data: &Xmb) -> Option<XmbFile> {
    // First find the nodes with no parents.
    // Then recursively add their children based on the parent index.
    // Assume a null pointer just means no entries.
    // TODO: Return an error instead of an option?
    let roots: Vec<_> = xmb_data.entries.0.as_ref().and_then(|entries| {
        entries
            .iter()
            .enumerate()
            .filter(|(_, e)| e.parent_index == -1)
            .map(|(i, e)| create_children_recursive(xmb_data, e, i as i16))
            .collect()
    })?;

    Some(XmbFile { entries: roots })
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
                            }],
                        },
                        XmbFileEntry {
                            name: "child2".into(),
                            attributes: indexmap!["a".into() => "5".into(), "b".into() => "6".into()],
                            children: Vec::new(),
                        }
                    ],
                }]
            },
            xmb_file
        );

        // Just test the tree representation to avoid testing formatting differences.
        let output_element = xmb_file.to_xml().unwrap();
        assert_eq!(element, output_element);
    }

    // TODO: Test xmb to xml_file
    #[test]
    fn xmb_file_to_xmb() {
        // TODO: Test mapped entries?
        let xmb_file = XmbFile {
            entries: vec![XmbFileEntry {
                name: "root".into(),
                attributes: indexmap!["a".into() => "1".into(), "b".into() => "2".into()],
                children: vec![
                    XmbFileEntry {
                        name: "child1".into(),
                        attributes: indexmap!["id".into() => "id2".into(), "b".into() => "4".into()],
                        children: vec![XmbFileEntry {
                            name: "subchild1".into(),
                            attributes: indexmap![
                                "c".into() => "7".into(),
                                "d".into() => "8".into(),
                                "e".into() => "f".into()
                            ],
                            children: Vec::new(),
                        }],
                    },
                    XmbFileEntry {
                        name: "child2".into(),
                        attributes: indexmap!["id".into() => "id1".into(), "b".into() => "6".into()],
                        children: Vec::new(),
                    },
                ],
            }],
        };

        // TODO: Use PartialEq for the entries, attributes, etc?
        let xmb = Xmb::from(&xmb_file);

        assert_eq!(4, xmb.entry_count);
        let entries = xmb.entries.0.as_ref().unwrap();
        assert_eq!(4, entries.len());
        // TODO: Document this order?
        assert_eq!(-1, entries[0].parent_index); // root
        assert_eq!(0, entries[1].parent_index); // child1
        assert_eq!(0, entries[2].parent_index); // child2
        assert_eq!(1, entries[3].parent_index); // subchild1

        assert_eq!(9, xmb.attribute_count);
        assert_eq!(9, xmb.attributes.0.as_ref().unwrap().len());

        assert_eq!(10, xmb.string_count);

        // child1 and child2 have "id" attributes.
        // The order is flipped here since the ids are sorted.
        assert_eq!(2, xmb.mapped_entry_count);
        let mapped_entries = xmb.mapped_entries.0.as_ref().unwrap();
        assert_eq!(2, mapped_entries[0].entry_index);
        assert_eq!(1, mapped_entries[1].entry_index);
    }

    #[test]
    fn write_read_xmbfile_nulls() {
        let xmb_file = XmbFile {
            entries: vec![XmbFileEntry {
                name: "\0".to_string(),
                attributes: indexmap!["\0".to_string() => "\0".to_string()],
                children: Vec::new(),
            }],
        };

        let mut writer = std::io::Cursor::new(Vec::new());
        xmb_file.write(&mut writer).unwrap();

        // Check that null bytes in the strings are removed.
        let mut reader = std::io::Cursor::new(writer.into_inner());
        let new_xmb_file = XmbFile::read(&mut reader).unwrap();
        assert_eq!(
            XmbFile {
                entries: vec![XmbFileEntry {
                    name: "".to_string(),
                    attributes: indexmap!["".to_string() => "".to_string()],
                    children: Vec::new(),
                }],
            },
            new_xmb_file
        );
    }

    #[test]
    fn xmb_file_to_xml_no_root() {
        let xmb_file = XmbFile {
            entries: Vec::new(),
        };

        let result = xmb_file.to_xml();
        assert!(matches!(result, Err(CreateXmlError::MissingRootElement)));
    }

    #[test]
    fn xmb_file_to_xml_multiple_root() {
        let xmb_file = XmbFile {
            entries: vec![
                XmbFileEntry {
                    name: String::new(),
                    attributes: IndexMap::new(),
                    children: Vec::new(),
                },
                XmbFileEntry {
                    name: String::new(),
                    attributes: IndexMap::new(),
                    children: Vec::new(),
                },
            ],
        };

        let result = xmb_file.to_xml();
        assert!(matches!(result, Err(CreateXmlError::MultipleRootElements)));
    }
}
