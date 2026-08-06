#[cfg(test)]
mod ssa_if_0 {
    const SOURCE: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/lang/tests/",
        "ssa_if_0.txt"
    ));

    #[test]
    fn ssa_if_0() {
        let mut out = vec![];

        let _ = apart::compile::<0, _>([(0, SOURCE)].as_slice(), &mut out)
            .expect("examples should always compile");

        assert_eq!(str::from_utf8(out.as_slice()), Ok("21\n"));
    }

    #[test]
    fn ssa_if_0_register() {
        let mut out = vec![];

        let _ = apart::compile::<32, _>([(0, SOURCE)].as_slice(), &mut out)
            .expect("examples should always compile");

        assert_eq!(str::from_utf8(out.as_slice()), Ok("21\n"));
    }
}

#[cfg(test)]
mod ssa_if_1 {
    const SOURCE: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/lang/tests/",
        "ssa_if_1.txt"
    ));

    #[test]
    fn ssa_if_1() {
        let mut out = vec![];

        let _ = apart::compile::<0, _>([(0, SOURCE)].as_slice(), &mut out)
            .expect("examples should always compile");

        assert_eq!(str::from_utf8(out.as_slice()), Ok("19\n"));
    }

    #[test]
    fn ssa_if_1_register() {
        let mut out = vec![];

        let _ = apart::compile::<32, _>([(0, SOURCE)].as_slice(), &mut out)
            .expect("examples should always compile");

        assert_eq!(str::from_utf8(out.as_slice()), Ok("19\n"));
    }
}

#[cfg(test)]
mod ssa_if_2 {
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
}

#[cfg(test)]
mod three_or_four {
    const SOURCE: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/lang/tests/",
        "three_or_four.txt"
    ));

    #[test]
    fn three_or_four() {
        let mut out = vec![];

        let _ = apart::compile::<0, _>([(0, SOURCE)].as_slice(), &mut out)
            .expect("examples should always compile");

        assert_eq!(str::from_utf8(out.as_slice()), Ok("3\n"));
    }

    #[test]
    fn three_or_four_register() {
        let mut out = vec![];

        let _ = apart::compile::<32, _>([(0, SOURCE)].as_slice(), &mut out)
            .expect("examples should always compile");

        assert_eq!(str::from_utf8(out.as_slice()), Ok("3\n"));
    }
}
