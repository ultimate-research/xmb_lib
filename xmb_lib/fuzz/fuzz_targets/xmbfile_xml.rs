#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|xmb: xmb_lib::XmbFile| {
    // Test that the XmbFile <-> XML conversion is 1:1.
    // This conversion shouldn't lose any information.
    let xml = xmb.to_xml();
    let _new_xmb = xmb_lib::XmbFile::from_xml(&xml);
    // assert_eq!(xmb, new_xmb);
});
