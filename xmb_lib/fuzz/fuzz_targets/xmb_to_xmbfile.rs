#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|xmb: xmb_lib::xmb::Xmb| {
    // Test that the Xmb -> XmbFile conversion doesn't panic.
    let _xmb_file = xmb_lib::XmbFile::from(&xmb);
});
