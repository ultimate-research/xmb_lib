#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // Add the magic to speed up generating test data.
    let mut xmb_data = Vec::with_capacity(data.len() + 4);
    xmb_data.extend_from_slice(b"XMB ");
    xmb_data.extend_from_slice(data);
    
    // Test that the Xmb parser doesn't panic.
    let mut reader = std::io::Cursor::new(data);
    xmb_lib::xmb::Xmb::read(&mut reader);
});
