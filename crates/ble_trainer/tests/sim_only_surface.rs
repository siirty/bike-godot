//! The "no ERG path" review, as a test: none of `trainer_core`'s non-SIM control-point writes may
//! be named anywhere in this crate's source. Reaching ERG would need a source change here, and
//! this fails it.

#[test]
fn no_non_sim_control_point_write_is_named_in_src() {
    let forbidden = [
        "set_target_power",
        "set_target_resistance_level",
        "set_wheel_circumference",
        "encode_set_target_power",
        "encode_set_target_resistance_level",
        "encode_set_wheel_circumference",
        "reset_frame",
        ".reset(",
    ];
    let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut checked = 0;
    for entry in std::fs::read_dir(&src).unwrap() {
        let path = entry.unwrap().path();
        let text = std::fs::read_to_string(&path).unwrap();
        // Doc text may name them to explain their absence; code may not.
        let code: String = text
            .lines()
            .filter(|l| !l.trim_start().starts_with("//"))
            .collect::<Vec<_>>()
            .join("\n");
        for symbol in forbidden {
            assert!(!code.contains(symbol), "{} names {symbol}", path.display());
        }
        checked += 1;
    }
    assert!(checked >= 5, "scanned the crate's modules");
}
