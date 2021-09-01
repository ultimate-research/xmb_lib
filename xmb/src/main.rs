use clap::{App, Arg};
use std::path::{Path, PathBuf};
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
                .help("The output XML or XMB file")
                .required(false)
                .takes_value(true),
        )
        .get_matches();

    let input = Path::new(matches.value_of("input").unwrap());

    // TODO: Clean this up.
    match input.extension().unwrap().to_str().unwrap() {
        "xml" => {
            let xml_text = std::io::Cursor::new(std::fs::read(input).unwrap());
            let element = Element::parse(xml_text).unwrap();
            let xmb_file = XmbFile::from_xml(&element);
            let xmb = Xmb::from(&xmb_file);

            print_tree(&xmb);

            // TODO: Just append xmb instead of adding .out?
            let output = matches
                .value_of("output")
                .map(|o| PathBuf::from(o))
                .unwrap_or(PathBuf::from(input).with_extension("out.xmb"));
            xmb.write_to_file(output).unwrap();
        }
        "xmb" => {
            let xmb = Xmb::from_file(input).unwrap();
            let xmb_file = XmbFile::from(&xmb);

            print_tree(&xmb);

            let element = xmb_file.to_xml();

            // Match the output of the original Python script where possible.
            let config = EmitterConfig::new()
                .perform_indent(true)
                .indent_string("    ")
                .pad_self_closing(false);

            let output = matches
                .value_of("output")
                .map(|o| PathBuf::from(o))
                .unwrap_or(PathBuf::from(input).with_extension("out.xml"));

            // Write the xml.
            let mut writer = std::io::BufWriter::new(std::fs::File::create(output).unwrap());
            element.write_with_config(&mut writer, config).unwrap();
        }
        _ => panic!("Unsupported extension"),
    }
}

fn print_tree(xmb: &Xmb) {
    // Print the tree structure for use with graphviz and the dot engine.
    // The output can be visualized here: https://edotor.net/
    println!("digraph G {{");
    println!("graph [ranksep=2];");
    for (i, entry) in xmb.entries.as_ref().unwrap().iter().enumerate() {
        // Add an edge from parent to child.
        if entry.parent_index != -1 {
            if let Some(parent) = xmb
                .entries
                .as_ref()
                .unwrap()
                .get(entry.parent_index as usize)
            {
                // Avoid adding duplicate edges.
                // if parent.unk1 != i as i16 {
                    println!(
                        r#""{}: {}" -> "{}: {}""#,
                        entry.parent_index,
                        xmb.read_name(parent.name_offset).unwrap(),
                        i,
                        xmb.read_name(entry.name_offset).unwrap()
                    );
                // }

            }
        }

        // Add the unk1 edges in a different color.
        if entry.unk1 < xmb.entry_count as i16 && entry.unk1 >= 0 {
            let next_node = &xmb.entries.as_ref().unwrap()[entry.unk1 as usize];
            println!(
                r#""{}: {}" -> "{}: {}" [color=blue]"#,
                i,
                xmb.read_name(entry.name_offset).unwrap(),
                entry.unk1,
                xmb.read_name(next_node.name_offset).unwrap()
            );
        } else {
            println!(
                r#""{}: {}" -> "{}" [color=blue]"#,
                i,
                xmb.read_name(entry.name_offset).unwrap(),
                entry.unk1
            );
        }
    }

    println!("}}");
}
