use binread::BinReaderExt;
use std::io::Write;
use std::time::Instant;
use std::{env, io::Cursor};
use xmb_lib::XmbFile;
use xmltree::EmitterConfig;

fn main() {
    // TODO: Clap for arguments.
    let args: Vec<String> = env::args().collect();
    // if args.len() != 2 {
    //     println!("Usage: smush_xmb.exe <xmb file>");
    //     return;
    // }

    let filename = &args[1];
    let output = &args[2];

    let parse_start_time = Instant::now();

    // let xmb = xmb_lib::read_xmb(Path::new(filename)).unwrap();
    let mut reader = std::io::Cursor::new(std::fs::read(filename).unwrap());
    let xmb: xmb_lib::xmb::Xmb = reader.read_le().unwrap();
    let parse_time = parse_start_time.elapsed();
    eprintln!("Parse: {:?}", parse_time);

    let xmb_file = XmbFile::from(&xmb);
    let element = xmb_file.to_xml();

    // Match the output of the original Python script where possible.
    let config = EmitterConfig::new()
        .perform_indent(true)
        .indent_string("    ")
        .pad_self_closing(false);

    let mut writer = std::io::Cursor::new(Vec::new());
    element.write_with_config(&mut writer, config).unwrap();

    let result = writer.into_inner();
    println!("{}", String::from_utf8(result).unwrap());

    let export_start_time = Instant::now();

    let mut cursor = Cursor::new(Vec::new());
    xmb.write(&mut cursor).unwrap();
    let mut output_file = std::fs::File::create(output).unwrap();
    output_file.write_all(cursor.get_mut()).unwrap();

    eprintln!("Export: {:?}", export_start_time.elapsed());
}
