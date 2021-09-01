use clap::{App, Arg};
use std::{
    io::{BufWriter, Write},
    path::{Path, PathBuf},
};
use xmb_lib::xmb::Xmb;
use xmb_lib::XmbFile;
use xmltree::{Element, EmitterConfig};

fn main() {
    let matches = App::new("xmb")
        .version("0.1")
        .author("SMG")
        .about("Convert XMB files to text formats")
        .arg(
            Arg::with_name("input")
                .index(1)
                .short("i")
                .long("input")
                .help("The input XML or XMB file")
                .required(true)
                .takes_value(true),
        )
        .arg(
            Arg::with_name("output")
                .index(2)
                .short("o")
                .long("output")
                .help("The output XML, XMB, or dot (graphviz) file")
                .required(false)
                .takes_value(true),
        )
        .get_matches();

    let input = matches.value_of("input").unwrap();

    // TODO: Clean this up.
    match PathBuf::from(&input).extension().unwrap().to_str().unwrap() {
        "xml" => {
            let xml_text = std::io::Cursor::new(std::fs::read(input).unwrap());
            let element = Element::parse(xml_text).unwrap();
            let xmb_file = XmbFile::from_xml(&element);
            let xmb = Xmb::from(&xmb_file);

            // Replace the xml extension.
            // Ex: model.xmb.xml -> model.xmb.xmb.
            let output = matches
                .value_of("output")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from(input).with_extension("xmb"));

            match output.extension().unwrap().to_str().unwrap() {
                "xmb" => xmb.write_to_file(output).unwrap(),
                "dot" => write_dot_graph(output, &xmb).unwrap(),
                _ => panic!("Unsupported output extension for XML input"),
            }
        }
        "xmb" => {
            let xmb = Xmb::from_file(input).unwrap();

            // Append .xml to the existing file to avoid overwriting existing files.
            // Ex: model.xmb -> model.xmb.xml.
            let output = matches
                .value_of("output")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from(input.to_string() + ".xml"));

            match output.extension().unwrap().to_str().unwrap() {
                "xmb" => xmb.write_to_file(output).unwrap(),
                "xml" => {
                    let xmb_file = XmbFile::from(&xmb);

                    let element = xmb_file.to_xml();

                    // Match the output of the original Python script where possible.
                    let config = EmitterConfig::new()
                        .perform_indent(true)
                        .indent_string("    ")
                        .pad_self_closing(false);

                    // Write the xml.
                    let mut writer =
                        std::io::BufWriter::new(std::fs::File::create(output).unwrap());
                    element.write_with_config(&mut writer, config).unwrap();
                }
                "dot" => write_dot_graph(output, &xmb).unwrap(),
                _ => panic!("Unsupported output extension for XMB input"),
            }
        }
        _ => panic!("Unsupported extension"),
    }
}

fn write_dot_graph<P: AsRef<Path>>(output: P, xmb: &Xmb) -> std::io::Result<()> {
    let mut file = BufWriter::new(std::fs::File::create(output)?);

    // Print the tree structure for use with graphviz and the dot engine.
    // The output can be visualized here: https://edotor.net/
    file.write_all(b"digraph G {\n")?;
    file.write_all(b"\tgraph [ranksep=2];\n")?;
    for (i, entry) in xmb.entries.as_ref().unwrap().iter().enumerate() {
        // Add an edge from parent to child.
        if entry.parent_index != -1 {
            if let Some(parent) = xmb
                .entries
                .as_ref()
                .unwrap()
                .get(entry.parent_index as usize)
            {
                // TODO: Avoid adding duplicate edges?
                // if parent.unk1 != i as i16 {
                writeln!(
                    &mut file,
                    "\t\"{}: {}\" -> \"{}: {}\"",
                    entry.parent_index,
                    xmb.read_name(parent.name_offset).unwrap(),
                    i,
                    xmb.read_name(entry.name_offset).unwrap()
                )?;
                // }
            }
        }

        // Add the unk1 edges in a different color.
        if entry.unk1 < xmb.entry_count as i16 && entry.unk1 >= 0 {
            let next_node = &xmb.entries.as_ref().unwrap()[entry.unk1 as usize];
            writeln!(
                &mut file,
                "\t\"{}: {}\" -> \"{}: {}\" [color=blue]",
                i,
                xmb.read_name(entry.name_offset).unwrap(),
                entry.unk1,
                xmb.read_name(next_node.name_offset).unwrap()
            )?;
        } else {
            writeln!(
                &mut file,
                "\t\"{}: {}\" -> \"{}\" [color=blue]",
                i,
                xmb.read_name(entry.name_offset).unwrap(),
                entry.unk1
            )?;
        }
    }

    file.write_all(b"}")?;
    file.flush()?;
    Ok(())
}
