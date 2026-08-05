const SOURCE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/lang/tests/",
    "invalid_assign_target.txt"
));

#[test]
fn invalid_assign_target() {
    let mut out = vec![];

    let compiled = apart::compile::<0, _>([(0, SOURCE)].as_slice(), &mut out);

    std::assert_matches!(compiled, Err(_));
}
