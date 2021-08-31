use std::env;
use std::io::{Cursor, Write};
use std::path::{Path, PathBuf};
use std::time::Instant;
use xmb_lib::XmbFile;
use xmb_lib::xmb::Xmb;
use xmltree::{Element, EmitterConfig};

// TODO: xml -> XmbFile -> Xmb
// Is the entry order just BFS starting from the root?

fn main() {
    // TODO: Clap for arguments.
    let args: Vec<String> = env::args().collect();
    let input = Path::new(&args[1]);

    // TODO: Clean this up.
    match input.extension().unwrap().to_str().unwrap() {
        "xml" => {
            let xml_text = std::io::Cursor::new(std::fs::read(input).unwrap());
            let element = Element::parse(xml_text).unwrap();
            let xmb_file = XmbFile::from_xml(&element);
            let xmb = Xmb::from(&xmb_file);

            for (i,entry) in xmb.entries.as_ref().unwrap().iter().enumerate() {
                println!("{:?}: {:?}", i, xmb.read_name(entry.name_offset).unwrap());
            }
            println!();

            for (i,entry) in xmb.entries.as_ref().unwrap().iter().enumerate() {
                // println!("{:?}: {:?}", i, xmb.read_name(entry.name_offset).unwrap());
                // println!("{:?} {:?}", i, entry.unk1);

                if entry.unk1 < xmb.entry_count as i16 && entry.unk1 >= 0 {
                    let next_node = &xmb.entries.as_ref().unwrap()[entry.unk1 as usize];
                    println!("{} : {} -> {} : {}", i, xmb.read_name(entry.name_offset).unwrap(), entry.unk1, xmb.read_name(next_node.name_offset).unwrap());
                } else {
                    println!("{} : {} -> {}", i, xmb.read_name(entry.name_offset).unwrap(), entry.unk1);
                }
                // println!();
            }

            // TODO: Just append xmb instead of adding .out?
            let output = PathBuf::from(input).with_extension("out.xmb");
            xmb.write_to_file(output).unwrap();
        }
        "xmb" => {
            let xmb = Xmb::from_file(input).unwrap();
            let xmb_file = XmbFile::from(&xmb);

            for (i,entry) in xmb.entries.as_ref().unwrap().iter().enumerate() {
                println!("{:?}: {:?}", i, xmb.read_name(entry.name_offset).unwrap());
            }
            println!();
            // println!("{:#?}", &xmb);
            // println!("{:#?}", &xmb_file);

            for (i,entry) in xmb.entries.as_ref().unwrap().iter().enumerate() {
                // println!("{:?}: {:?}", i, xmb.read_name(entry.name_offset).unwrap());
                // println!("{:?} {:?}", i, entry.unk1);

                if entry.unk1 < xmb.entry_count as i16 && entry.unk1 >= 0 {
                    let next_node = &xmb.entries.as_ref().unwrap()[entry.unk1 as usize];
                    println!("{} : {} -> {} : {}", i, xmb.read_name(entry.name_offset).unwrap(), entry.unk1, xmb.read_name(next_node.name_offset).unwrap());
                } else {
                    println!("{} : {} -> {}", i, xmb.read_name(entry.name_offset).unwrap(), entry.unk1);
                }
                // println!();
            }

            let element = xmb_file.to_xml();

            // Match the output of the original Python script where possible.
            let config = EmitterConfig::new()
                .perform_indent(true)
                .indent_string("    ")
                .pad_self_closing(false);
        
            let mut writer = std::io::Cursor::new(Vec::new());
            element.write_with_config(&mut writer, config).unwrap();
        
            // Write the xml.
            // TODO: Just append xml instead of adding .out?
            let output = PathBuf::from(input).with_extension("out.xml");
            let mut output_file = std::fs::File::create(output).unwrap();
            output_file.write_all(writer.get_mut()).unwrap();
        }
        _ => panic!("Unsupported extension")
    }
}
