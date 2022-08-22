use arbitrary::Arbitrary;
use binrw::{
    helpers::until_eof, BinRead, BinReaderExt, BinResult, NullString, ReadOptions, VecArgs,
};
use ssbh_lib::Ptr32;
use ssbh_write::SsbhWrite;
use std::{
    io::{Cursor, Read, Seek, SeekFrom, Write},
    path::Path,
};
// TODO: Limit the number of nodes to fall within the appropriate ranges?
// This is limited by the number of bits for the indices rather than entry count.
// TODO: Document remaining fields.
// TODO: bigendian if node_count has FF000000 > 0?

/// A flattened tree of named nodes with each node containing a collection of named attributes.
/// This corresponds to an XML document.
#[derive(Debug, BinRead, SsbhWrite, Arbitrary)]
#[br(magic = b"XMB ")]
#[ssbhwrite(align_after = 4)]
pub struct Xmb {
    pub entry_count: u32,
    pub attribute_count: u32,
    pub string_count: u32,
    pub mapped_entry_count: u32,

    /// Offsets for the values in [string_values](struct.Xmb.html#structfield.string_values) sorted alphabetically.
    #[br(count = string_count)]
    pub string_offsets: XmbVec<u32>, // sorted in alphabetical order by string

    /// A flattened list of entries.
    #[br(count = entry_count)]
    pub entries: XmbVec<Entry>,

    /// A combined collection of all [Entry] attributes.
    #[br(count = attribute_count)]
    pub attributes: XmbVec<Attribute>,

    /// A lookup table for the `"id"` attribute sorted alphabetically by value.
    #[br(count = mapped_entry_count)]
    pub mapped_entries: XmbVec<MappedEntry>,

    /// Unique values for [Entry] and [Attribute] names.
    #[br(args(string_count))]
    pub string_names: Ptr32<NamesBuffer>,

    /// Unique values for [Attribute] values.
    #[ssbhwrite(pad_after = 20)]
    pub string_values: Ptr32<ValuesBuffer>,
}

/// A named node with a collection of named attributes that corresponds to an XML element.
/// The [parent_index](#structfield.parent_index) can be used to recreate the original tree structure.
#[derive(Debug, BinRead, SsbhWrite, Arbitrary)]
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
#[derive(Debug, BinRead, SsbhWrite, Arbitrary)]
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
#[derive(Debug, BinRead, SsbhWrite, Arbitrary)]
pub struct MappedEntry {
    /// The offset in [string_values](struct.Xmb.html#structfield.string_values) for the `"id"` value.
    pub value_offset: u32,
    /// The index of the corresponding [Entry] in [entries](struct.Xmb.html#structfield.entries).
    pub entry_index: u32,
}

#[derive(Debug, SsbhWrite, Arbitrary)]
#[ssbhwrite(alignment = 4)]
pub struct NamesBuffer(pub Vec<u8>);

impl BinRead for NamesBuffer {
    // The names buffer has a string count.
    // This essentially counts the number of null bytes.
    type Args = (u32,);

    fn read_options<R: Read + Seek>(
        reader: &mut R,
        _options: &ReadOptions,
        args: Self::Args,
    ) -> BinResult<Self> {
        // TODO: Avoid redundant reads of strings?
        let mut values = Vec::new();
        let expected_count = args.0;

        let mut current_count = 0;
        loop {
            if current_count == expected_count {
                break;
            }

            let val: u8 = reader.read_le()?;
            if val == 0 {
                current_count += 1;
            }
            values.push(val);
        }
        Ok(Self(values))
    }
}

// The values buffer has no count and fills the rest of the file.
#[derive(Debug, BinRead, SsbhWrite, Arbitrary)]
#[ssbhwrite(alignment = 4)]
pub struct ValuesBuffer(#[br(parse_with = until_eof)] pub Vec<u8>);

#[derive(Debug, SsbhWrite, Arbitrary)]
pub struct XmbVec<T: SsbhWrite>(pub Ptr32<Vec<T>>);

impl<T: SsbhWrite> XmbVec<T> {
    pub fn new(elements: Vec<T>) -> Self {
        Self(Ptr32::new(elements))
    }
}

impl<T: BinRead<Args = ()> + SsbhWrite> BinRead for XmbVec<T> {
    type Args = VecArgs<()>;

    fn read_options<R: Read + Seek>(
        reader: &mut R,
        options: &ReadOptions,
        args: Self::Args,
    ) -> BinResult<Self> {
        let offset = u32::read_options(reader, options, ())?;
        if offset == 0 {
            return Ok(XmbVec(Ptr32::null()));
        }
        let saved_pos = reader.stream_position()?;

        // Create a custom reader to avoid preallocating memory.
        // This makes the parser more resilient to malformed length fields.
        // Xmb files tend to have a small number of entries anyway.
        reader.seek(SeekFrom::Start(offset as u64))?;
        let mut elements = Vec::new();
        for _ in 0..args.count {
            let element = T::read_options(reader, options, ())?;
            elements.push(element);
        }

        reader.seek(SeekFrom::Start(saved_pos))?;
        Ok(XmbVec(Ptr32::new(elements)))
    }
}

impl Xmb {
    pub fn read_name(&self, offset: u32) -> Option<String> {
        let mut reader = Cursor::new(&self.string_names.as_ref()?.0);
        reader.seek(SeekFrom::Start(offset as u64)).ok()?;
        NullString::read(&mut reader).ok().map(|s| s.into_string())
    }

    pub fn read_value(&self, offset: u32) -> Option<String> {
        let mut reader = Cursor::new(&self.string_values.as_ref()?.0);
        reader.seek(SeekFrom::Start(offset as u64)).ok()?;
        NullString::read(&mut reader).ok().map(|s| s.into_string())
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
        // TODO: Magic support for SsbhWrite?
        writer.write_all(b"XMB ")?;
        let mut data_ptr = 4;
        self.ssbh_write(writer, &mut data_ptr)?;
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
