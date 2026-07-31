const SOURCE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/lang/tests/",
    "fn_parameter.txt"
));

#[test]
fn fn_parameter() {
    let mut out = vec![];

    let _ =
        apart::compile([(0, SOURCE)].as_slice(), &mut out).expect("examples should always compile");

    assert_eq!(str::from_utf8(out.as_slice()), Ok("21\n"));
}
