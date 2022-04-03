#![no_main]
use libfuzzer_sys::fuzz_target;
use std::convert::TryFrom;

fuzz_target!(|xmb: xmb_lib::xmb::Xmb| {
    // Test that the Xmb -> XmbFile conversion doesn't panic.
    let _ = xmb_lib::XmbFile::try_from(xmb);
});
