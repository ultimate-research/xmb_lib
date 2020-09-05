use std::env;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        println!("Usage: smush_xmb.exe <xmb file>");
        return;
    }

    let filename = &args[1];

    let xmb = xmb_lib::read_xmb(filename).unwrap();
    let json = serde_json::to_string_pretty(&xmb).unwrap();
    println!("{}", json);
}
