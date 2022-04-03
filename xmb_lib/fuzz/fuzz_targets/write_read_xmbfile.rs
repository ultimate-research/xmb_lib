#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|xmb_file: xmb_lib::XmbFile| {
    // Writing the xmb file to binary.
    let mut writer = std::io::Cursor::new(Vec::new());
    xmb_file.write(&mut writer).unwrap();

    // Reading the data should give the original xmb.
    let mut reader = std::io::Cursor::new(writer.into_inner());
    let new_xmb_file = xmb_lib::XmbFile::read(&mut reader).unwrap();
    assert_eq!(new_xmb_file, xmb_file);
});
