use binrw::{
    args, helpers::until_eof, BinRead, BinReaderExt, BinResult, Endian, FilePtr32, NullString,
};
use std::{
    io::{Cursor, Read, Seek, SeekFrom, Write},
    path::Path,
};
use xc3_write::{write_full, Xc3Write, Xc3WriteOffsets};
// TODO: Limit the number of nodes to fall within the appropriate ranges?
// This is limited by the number of bits for the indices rather than entry count.
// TODO: Document remaining fields.
// TODO: bigendian if node_count has FF000000 > 0?

/// A flattened tree of named nodes with each node containing a collection of named attributes.
/// This corresponds to an XML document.
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, BinRead, Xc3Write, Xc3WriteOffsets)]
#[br(magic(b"XMB "))]
#[xc3(magic(b"XMB "))]
#[xc3(align_after(4))]
pub struct Xmb {
    pub entry_count: u32,
    pub attribute_count: u32,
    pub string_count: u32,
    pub mapped_entry_count: u32,

    /// Offsets for the values in [string_values](struct.Xmb.html#structfield.string_values) sorted alphabetically.
    #[br(parse_with = FilePtr32::parse)]
    #[br(args { inner: args! { count: string_count as usize } })]
    #[xc3(offset(u32))]
    pub string_offsets: Vec<u32>, // sorted in alphabetical order by string

    /// A flattened list of entries.
    #[br(parse_with = FilePtr32::parse)]
    #[br(args { inner: args! { count: entry_count as usize } })]
    #[xc3(offset(u32))]
    pub entries: Vec<Entry>,

    /// A combined collection of all [Entry] attributes.
    #[br(parse_with = FilePtr32::parse)]
    #[br(args { inner: args! { count: attribute_count as usize } })]
    #[xc3(offset(u32))]
    pub attributes: Vec<Attribute>,

    /// A lookup table for the `"id"` attribute sorted alphabetically by value.
    #[br(parse_with = FilePtr32::parse)]
    #[br(args { inner: args! { count: mapped_entry_count as usize } })]
    #[xc3(offset(u32))]
    pub mapped_entries: Vec<MappedEntry>,

    /// Unique values for [Entry] and [Attribute] names.
    #[br(parse_with = FilePtr32::parse)]
    #[br(args { inner: string_count })]
    #[xc3(offset(u32), align(4))]
    pub string_names: NamesBuffer,

    /// Unique values for [Attribute] values.
    #[br(parse_with = FilePtr32::parse)]
    #[xc3(offset(u32), align(4))]
    pub string_values: ValuesBuffer,

    // TODO: Padding?
    pub unks: [u32; 5],
}

/// A named node with a collection of named attributes that corresponds to an XML element.
/// The [parent_index](#structfield.parent_index) can be used to recreate the original tree structure.
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, BinRead, Xc3Write, Xc3WriteOffsets)]
pub struct Entry {
    pub name_offset: u32,
    pub attribute_count: u16,
    pub child_count: u16,
    pub attribute_start_index: i16,
    pub unk1: i16, // TODO: Is there a name for this traversal?
    /// The index of the parent [Entry] in [entries](struct.Xmb.html#structfield.entries) or `-1` if there is no parent.
    pub parent_index: i16,
    pub unk2: i16, // always -1?
}

/// A key value pair that corresponds to an XML attribute.
/// # Examples
/// The `value_offset` would be the offset of `"eff_elec01"` in the string values section.
/**
```xml
<entry id="eff_elec01"/>
```
 */
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, BinRead, Xc3Write, Xc3WriteOffsets)]
pub struct Attribute {
    pub name_offset: u32,
    pub value_offset: u32,
}

/// An element of the `"id"` attribute lookup for an [Entry].
/// # Examples
/**
```xml
<entry id="eff_elec01"/>
```
 */
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, BinRead, Xc3Write, Xc3WriteOffsets)]
pub struct MappedEntry {
    /// The offset in [string_values](struct.Xmb.html#structfield.string_values) for the `"id"` value.
    pub value_offset: u32,
    /// The index of the corresponding [Entry] in [entries](struct.Xmb.html#structfield.entries).
    pub entry_index: u32,
}

#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Xc3Write, Xc3WriteOffsets)]
pub struct NamesBuffer {
    pub data: Vec<u8>,
}

impl BinRead for NamesBuffer {
    // The names buffer has a string count.
    // This essentially counts the number of null bytes.
    type Args<'a> = u32;

    fn read_options<R: Read + Seek>(
        reader: &mut R,
        _endian: Endian,
        args: Self::Args<'_>,
    ) -> BinResult<Self> {
        // TODO: Avoid redundant reads of strings?
        let mut data = Vec::new();
        let expected_count = args;

        let mut current_count = 0;
        loop {
            if current_count == expected_count {
                break;
            }

            let val: u8 = reader.read_le()?;
            if val == 0 {
                current_count += 1;
            }
            data.push(val);
        }
        Ok(Self { data })
    }
}

// The values buffer has no count and fills the rest of the file.
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, BinRead, Xc3Write, Xc3WriteOffsets)]
pub struct ValuesBuffer {
    #[br(parse_with = until_eof)]
    pub data: Vec<u8>,
}

impl Xmb {
    pub fn read_name(&self, offset: u32) -> Option<String> {
        let mut reader = Cursor::new(&self.string_names.data);
        reader.seek(SeekFrom::Start(offset as u64)).ok()?;
        NullString::read(&mut reader).ok().map(|s| s.to_string())
    }

    pub fn read_value(&self, offset: u32) -> Option<String> {
        let mut reader = Cursor::new(&self.string_values.data);
        reader.seek(SeekFrom::Start(offset as u64)).ok()?;
        NullString::read(&mut reader).ok().map(|s| s.to_string())
    }

    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self, Box<dyn std::error::Error>> {
        // Buffer the entire file for performance since most XMB files are small.
        let mut reader = std::io::Cursor::new(std::fs::read(path).unwrap());
        let xmb: Xmb = reader.read_le()?;
        Ok(xmb)
    }

    pub fn read<R: Read + Seek>(reader: &mut R) -> Result<Self, Box<dyn std::error::Error>> {
        // TODO: Not all xmb files are little endian.
        let xmb: Xmb = reader.read_le()?;
        Ok(xmb)
    }

    pub fn write<W: Write + Seek>(&self, writer: &mut W) -> std::io::Result<()> {
        write_full(self, writer, 0, &mut 0)?;
        Ok(())
    }

    pub fn write_to_file<P: AsRef<Path>>(&self, path: P) -> std::io::Result<()> {
        let mut writer = Cursor::new(Vec::new());
        self.write(&mut writer)?;
        let mut output = std::fs::File::create(path)?;
        output.write_all(writer.get_mut())?;
        Ok(())
    }
}
