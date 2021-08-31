use binread::{BinRead, BinReaderExt, BinResult, NullString, ReadOptions};
use ssbh_lib::Ptr32;
use ssbh_write::SsbhWrite;
use std::{
    io::{Cursor, Read, Seek, Write},
    num::NonZeroU8,
    path::Path,
};

// TODO: Limit the number of nodes to fall within the appropriate ranges?
// This is limited by the number of bits for the indices rather than entry count.

// TODO: Document remaining fields.

/// A named node with a collection of named attributes that corresponds to an XML element.
/// The [parent_index](#structfield.parent_index) can be used to recreate the original tree structure.
#[derive(BinRead, Debug, SsbhWrite)]
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
#[derive(BinRead, Debug, SsbhWrite)]
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
#[derive(BinRead, Debug, SsbhWrite)]
pub struct MappedEntry {
    /// The offset in [string_values](struct.Xmb.html#structfield.string_values) for the `"id"` value.
    pub value_offset: u32,
    /// The index of the corresponding [Entry] in [entries](struct.Xmb.html#structfield.entries).
    pub entry_index: u32,
}

// TODO: bigendian if node_count has FF000000 > 0?

/// A flattened tree of named nodes with each node containing a collection of named attributes.
/// This corresponds to an XML document.
#[derive(BinRead, Debug, SsbhWrite)]
#[br(magic = b"XMB ")]
#[ssbhwrite(align_after = 4)]
pub struct Xmb {
    pub entry_count: u32,
    pub attribute_count: u32,
    pub string_count: u32,
    pub mapped_entry_count: u32,

    /// Offsets for the values in [string_values](struct.Xmb.html#structfield.string_values) sorted alphabetically.
    #[br(count = string_count)]
    pub string_offsets: Ptr32<Vec<u32>>, // sorted in alphabetical order by string

    /// A flattened list of entries.
    #[br(count = entry_count)]
    pub entries: Ptr32<Vec<Entry>>,

    /// A combined collection of all [Entry] attributes.
    #[br(count = attribute_count)]
    pub attributes: Ptr32<Vec<Attribute>>,

    /// A lookup table for the `"id"` attribute sorted alphabetically by value.
    #[br(count = mapped_entry_count)]
    pub mapped_entries: Ptr32<Vec<MappedEntry>>,

    /// Unique values for [Entry] and [Attribute] names.
    #[br(count = string_count)]
    pub string_names: Ptr32<StringBuffer>,

    /// Unique values for [Attribute] values.
    pub string_values: Ptr32<StringBuffer>,

    // TODO: add align_after support per field for SsbhWrite
    pub padding1: u32,
    pub padding2: u128, // TODO: Is the header always padded to 64 bytes?
}

impl Xmb {
    pub fn read_name(&self, name_offset: u32) -> Option<String> {
        // TODO: This is a messy way to find the string with the given offset.
        self.string_names
            .as_ref()?
            .0
            .iter()
            .find(|(offset, _)| *offset == name_offset as u64)
            .map(|(_, value)| value.to_string())
    }

    pub fn read_value(&self, value_offset: u32) -> Option<String> {
        self.string_values
            .as_ref()?
            .0
            .iter()
            .find(|(offset, _)| *offset == value_offset as u64)
            .map(|(_, value)| value.to_string())
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

#[derive(Debug)]
pub struct StringBuffer(pub Vec<(u64, NullString)>);

impl BinRead for StringBuffer {
    type Args = ();

    fn read_options<R: Read + Seek>(
        reader: &mut R,
        options: &ReadOptions,
        _args: Self::Args,
    ) -> BinResult<Self> {
        let mut values = Vec::new();
        // The string names have a specified count.
        if let Some(count) = options.count {
            let start = reader.stream_position()?;
            for _ in 0..count {
                let relative_offset = reader.stream_position()? - start;
                let value: NullString = reader.read_le()?;
                values.push((relative_offset, value));
            }
        } else {
            // HACK: Create a second buffer to be able to detect EOF.
            // There's probably a nicer way of reading null terminated strings in a loop.
            // TODO: Check if the reader position hasn't moved since the last iteration instead?
            let mut buffer = Vec::new();
            reader.read_to_end(&mut buffer)?;

            // This case is just to handle the string values going until EOF.
            // The names buffer has a specified string count.
            let mut buffer_reader = Cursor::new(buffer);
            loop {
                let relative_offset = (&mut buffer_reader).stream_position()?;
                if relative_offset as usize >= (&mut buffer_reader).get_ref().len() {
                    break;
                }
                let byte_result: Result<Vec<u8>, _> = (&mut buffer_reader)
                    .bytes()
                    .take_while(|b| !matches!(b, Ok(0)))
                    .collect();
                let bytes: Vec<_> = byte_result?
                    .into_iter()
                    .map(|x| unsafe { NonZeroU8::new_unchecked(x) })
                    .collect();
                values.push((relative_offset, bytes.into()));
            }
        }

        Ok(Self(values))
    }
}

// TODO: SsbhWrite lacks a skip attribute for the saved position, so this can't be derived.
impl SsbhWrite for StringBuffer {
    fn ssbh_write<W: std::io::Write + std::io::Seek>(
        &self,
        writer: &mut W,
        data_ptr: &mut u64,
    ) -> std::io::Result<()> {
        let current_pos = writer.stream_position()?;
        if *data_ptr < current_pos + self.size_in_bytes() {
            *data_ptr = current_pos + self.size_in_bytes();
        }

        // Write each string and null terminator.
        for (_, value) in &self.0 {
            writer.write_all(&value.0)?;
            writer.write_all(&[0u8])?;
        }

        Ok(())
    }

    fn size_in_bytes(&self) -> u64 {
        self.0.len() as u64
    }

    fn alignment_in_bytes() -> u64 {
        4
    }
}
