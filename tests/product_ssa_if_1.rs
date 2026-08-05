const SOURCE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/lang/tests/",
    "product_ssa_if_1.txt"
));

#[test]
fn product_ssa_if_1() {
    let mut out = vec![];

    let _ =
        apart::compile([(0, SOURCE)].as_slice(), &mut out).expect("examples should always compile");

    assert_eq!(str::from_utf8(out.as_slice()), Ok("19\n"));
}
