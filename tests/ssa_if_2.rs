const SOURCE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/lang/tests/",
    "ssa_if_2.txt"
));

#[test]
fn ssa_if_2() {
    let mut out = vec![];

    let _ = apart::compile::<0, _>([(0, SOURCE)].as_slice(), &mut out)
        .expect("examples should always compile");

    assert_eq!(str::from_utf8(out.as_slice()), Ok("38\n"));
}

#[test]
fn ssa_if_2_register() {
    let mut out = vec![];

    let _ = apart::compile::<32, _>([(0, SOURCE)].as_slice(), &mut out)
        .expect("examples should always compile");

    assert_eq!(str::from_utf8(out.as_slice()), Ok("38\n"));
}
