use std::env;
use std::path::Path;
use std::time::Instant;

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

    let json = serde_json::to_string_pretty(&xmb).unwrap();
    println!("{}", json);
}
