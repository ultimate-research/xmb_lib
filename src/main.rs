use std::env;
use std::path::Path;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        println!("Usage: smush_xmb.exe <xmb file>");
        return;
    }

    let filename = &args[1];

    let xmb = xmb_lib::read_xmb(Path::new(filename)).unwrap();
    let json = serde_json::to_string_pretty(&xmb).unwrap();
    println!("{}", json);
}
