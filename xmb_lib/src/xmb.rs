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
#[derive(BinRead, Debug, SsbhWrite)]
pub struct Entry {
    pub name_offset: u32,
    pub property_count: u16,
    pub child_count: u16,
    pub property_start_index: i16,
    pub unk1: u16, // TODO: first child index or start of next child group
    pub parent_index: i16,
    pub unk2: i16, // always -1?
}

#[derive(BinRead, Debug, SsbhWrite)]
pub struct Property {
    pub name_offset: u32,
    pub value_offset: u32,
}

// TODO: This is probably an entityreference since XMB files with mapped entries define an id.
// TODO: Does this add a reference to value_offset to the entry at index unk_index?
// TODO: Add an idref attribute or create a new node?
#[derive(BinRead, Debug, SsbhWrite)]
pub struct MappedEntry {
    pub value_offset: u32,
    pub unk_index: u32, // parent entry?
}

// TODO: bigendian if node_count has FF000000 > 0?
#[derive(BinRead, Debug, SsbhWrite)]
#[br(magic = b"XMB ")]
#[ssbhwrite(align_after = 4)]
pub struct Xmb {
    pub entry_count: u32,
    pub property_count: u32,
    pub string_count: u32,
    pub mapped_entry_count: u32,

    // TODO: Use this to cache the string lookups (names only)?
    // TODO: This seems to be all the names in the string buffer, not including values.
    #[br(count = string_count)]
    pub string_offsets: Ptr32<Vec<u32>>, // sorted in alphabetical order by string

    #[br(count = entry_count)]
    pub entries: Ptr32<Vec<Entry>>,

    #[br(count = property_count)]
    pub properties: Ptr32<Vec<Property>>,

    // TODO: These values are sorted alphabetically.
    // This is likely just some sort of lookup.
    #[br(count = mapped_entry_count)]
    pub mapped_entries: Ptr32<Vec<MappedEntry>>,

    #[br(count = string_count)]
    pub string_names: Ptr32<StringBuffer>,

    // TODO: Not specifying count is a confusing way to specify read to eof.
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
