use lumetrope_core::archive::{pack, unpack};

#[test]
fn zip_round_trip_keeps_names_order_and_bytes() {
    let entries = vec![("manifest.json".to_string(), b"{\"a\":1}".to_vec()), ("projects/x/history.bin".to_string(), (0..=255u8).collect()), ("empty.txt".to_string(), vec![])];
    let z = pack(&entries).unwrap();
    assert_eq!(unpack(&z).unwrap(), entries);
    assert!(unpack(b"not a zip").is_err());
}
