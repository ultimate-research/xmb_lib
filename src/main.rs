use std::env;
use std::path::Path;
use std::time::Instant;

use xmb_lib::{XmbFile, XmbFileEntry};
use xmltree::{Element, EmitterConfig, XMLNode};

fn create_element_recursive(xmb: &XmbFile, entry: &XmbFileEntry) -> Element {
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
        // TODO: IndexMap to preserve order.
        attributes: entry.attributes.clone(),
        children,
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        println!("Usage: smush_xmb.exe <xmb file>");
        return;
    }

    let filename = &args[1];

    let parse_start_time = Instant::now();

    let xmb = xmb_lib::read_xmb(Path::new(filename)).unwrap();
    let parse_time = parse_start_time.elapsed();
    eprintln!("Parse: {:?}", parse_time);

    // TODO: Make this a method on XmbFile.
    // TODO: Don't assume this is the root entry or that there is a single root?
    let entry = &xmb.entries[0];
    let element = create_element_recursive(&xmb, entry);

    // Match the output of the original Python script where possible.
    let config = EmitterConfig::new()
        .perform_indent(true)
        .indent_string("    ")
        .pad_self_closing(false);

    let mut writer = std::io::Cursor::new(Vec::new());
    element.write_with_config(&mut writer, config).unwrap();

    let result = writer.into_inner();
    println!("{}", String::from_utf8(result).unwrap());
}
