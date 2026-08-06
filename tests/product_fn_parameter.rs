const SOURCE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/lang/tests/",
    "product_fn_parameter.txt"
));

#[test]
fn product_fn_parameter() {
    let mut out = vec![];

    let _ = apart::compile::<0, _>([(0, SOURCE)].as_slice(), &mut out)
        .expect("examples should always compile");

    assert_eq!(str::from_utf8(out.as_slice()), Ok("{}\n"));
}

#[test]
fn product_fn_parameter_register() {
    let mut out = vec![];

    let _ = apart::compile::<32, _>([(0, SOURCE)].as_slice(), &mut out)
        .expect("examples should always compile");

    assert_eq!(str::from_utf8(out.as_slice()), Ok("{}\n"));
}
