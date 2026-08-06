const SOURCE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/lang/tests/",
    "product_return_type.txt"
));

#[test]
fn product_return_type() {
    let mut out = vec![];

    let _ = apart::compile::<0, _>([(0, SOURCE)].as_slice(), &mut out)
        .expect("examples should always compile");

    assert_eq!(str::from_utf8(out.as_slice()), Ok("{}\n"));
}

#[test]
fn product_return_type_register() {
    let mut out = vec![];

    let _ = apart::compile::<32, _>([(0, SOURCE)].as_slice(), &mut out)
        .expect("examples should always compile");

    assert_eq!(str::from_utf8(out.as_slice()), Ok("{}\n"));
}
