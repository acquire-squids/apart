const SOURCE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/lang/tests/",
    "generic_equality.txt"
));

#[test]
fn generic_equality() {
    let mut out = vec![];

    let compiled = apart::compile::<0, _>([(0, SOURCE)].as_slice(), &mut out);

    std::assert_matches!(compiled, Err(_));
}
