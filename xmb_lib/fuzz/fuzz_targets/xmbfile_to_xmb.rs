#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|xmb_file: xmb_lib::XmbFile| {
    // Test that the XmbFile -> Xmb conversion doesn't panic.
    let _xmb = xmb_lib::xmb::Xmb::from(&xmb_file);
});
