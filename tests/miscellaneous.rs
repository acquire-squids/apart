#[cfg(test)]
mod and {
    const SOURCE: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/lang/tests/",
        "and.txt"
    ));

    #[test]
    fn and() {
        let mut out = vec![];

        let _ = apart::compile::<0, _>([(0, SOURCE)].as_slice(), &mut out)
            .expect("examples should always compile");

        assert_eq!(str::from_utf8(out.as_slice()), Ok("true\nfalse\n"));
    }

    #[test]
    fn and_register() {
        let mut out = vec![];

        let _ = apart::compile::<32, _>([(0, SOURCE)].as_slice(), &mut out)
            .expect("examples should always compile");

        assert_eq!(str::from_utf8(out.as_slice()), Ok("true\nfalse\n"));
    }
}

#[cfg(test)]
mod and_and_and_and_and {
    const SOURCE: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/lang/tests/",
        "and_and_and_and_and.txt"
    ));

    #[test]
    fn and_and_and_and_and() {
        let mut out = vec![];

        let _ = apart::compile::<0, _>([(0, SOURCE)].as_slice(), &mut out)
            .expect("examples should always compile");

        assert_eq!(str::from_utf8(out.as_slice()), Ok("true\n"));
    }

    #[test]
    fn and_and_and_and_and_register() {
        let mut out = vec![];

        let _ = apart::compile::<32, _>([(0, SOURCE)].as_slice(), &mut out)
            .expect("examples should always compile");

        assert_eq!(str::from_utf8(out.as_slice()), Ok("true\n"));
    }
}

#[cfg(test)]
mod and_and_or {
    const SOURCE: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/lang/tests/",
        "and_and_or.txt"
    ));

    #[test]
    fn and_and_or() {
        let mut out = vec![];

        let _ = apart::compile::<0, _>([(0, SOURCE)].as_slice(), &mut out)
            .expect("examples should always compile");

        assert_eq!(str::from_utf8(out.as_slice()), Ok("true\n"));
    }

    #[test]
    fn and_and_or_register() {
        let mut out = vec![];

        let _ = apart::compile::<32, _>([(0, SOURCE)].as_slice(), &mut out)
            .expect("examples should always compile");

        assert_eq!(str::from_utf8(out.as_slice()), Ok("true\n"));
    }
}

#[cfg(test)]
mod empty {
    const SOURCE: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/lang/tests/",
        "empty.txt"
    ));

    #[test]
    fn empty() {
        let mut out = vec![];

        let _ = apart::compile::<0, _>([(0, SOURCE)].as_slice(), &mut out)
            .expect("examples should always compile");

        assert_eq!(str::from_utf8(out.as_slice()), Ok(""));
    }

    #[test]
    fn empty_register() {
        let mut out = vec![];

        let _ = apart::compile::<32, _>([(0, SOURCE)].as_slice(), &mut out)
            .expect("examples should always compile");

        assert_eq!(str::from_utf8(out.as_slice()), Ok(""));
    }
}

#[cfg(test)]
mod fn_parameter {
    const SOURCE: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/lang/tests/",
        "fn_parameter.txt"
    ));

    #[test]
    fn fn_parameter() {
        let mut out = vec![];

        let _ = apart::compile::<0, _>([(0, SOURCE)].as_slice(), &mut out)
            .expect("examples should always compile");

        assert_eq!(str::from_utf8(out.as_slice()), Ok("21\n"));
    }

    #[test]
    fn fn_parameter_register() {
        let mut out = vec![];

        let _ = apart::compile::<32, _>([(0, SOURCE)].as_slice(), &mut out)
            .expect("examples should always compile");

        assert_eq!(str::from_utf8(out.as_slice()), Ok("21\n"));
    }
}

#[cfg(test)]
mod r#if {
    const SOURCE: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/lang/tests/",
        "if.txt"
    ));

    #[test]
    fn r#if() {
        let mut out = vec![];

        let _ = apart::compile::<0, _>([(0, SOURCE)].as_slice(), &mut out)
            .expect("examples should always compile");

        assert_eq!(str::from_utf8(out.as_slice()), Ok("21.0\n"));
    }

    #[test]
    fn r#if_register() {
        let mut out = vec![];

        let _ = apart::compile::<32, _>([(0, SOURCE)].as_slice(), &mut out)
            .expect("examples should always compile");

        assert_eq!(str::from_utf8(out.as_slice()), Ok("21.0\n"));
    }
}

#[cfg(test)]
mod or {
    const SOURCE: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/lang/tests/",
        "or.txt"
    ));

    #[test]
    fn or() {
        let mut out = vec![];

        let _ = apart::compile::<0, _>([(0, SOURCE)].as_slice(), &mut out)
            .expect("examples should always compile");

        assert_eq!(str::from_utf8(out.as_slice()), Ok("true\nfalse\n"));
    }

    #[test]
    fn or_register() {
        let mut out = vec![];

        let _ = apart::compile::<32, _>([(0, SOURCE)].as_slice(), &mut out)
            .expect("examples should always compile");

        assert_eq!(str::from_utf8(out.as_slice()), Ok("true\nfalse\n"));
    }
}

#[cfg(test)]
mod or_and_or_and_or {
    const SOURCE: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/lang/tests/",
        "or_and_or_and_or.txt"
    ));

    #[test]
    fn or_and_or_and_or() {
        let mut out = vec![];

        let _ = apart::compile::<0, _>([(0, SOURCE)].as_slice(), &mut out)
            .expect("examples should always compile");

        assert_eq!(str::from_utf8(out.as_slice()), Ok("true\n"));
    }

    #[test]
    fn or_and_or_and_or_register() {
        let mut out = vec![];

        let _ = apart::compile::<32, _>([(0, SOURCE)].as_slice(), &mut out)
            .expect("examples should always compile");

        assert_eq!(str::from_utf8(out.as_slice()), Ok("true\n"));
    }
}

#[cfg(test)]
mod equality {
    const SOURCE: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/lang/tests/",
        "equality.txt"
    ));

    #[test]
    fn three_or_four() {
        let mut out = vec![];

        let _ = apart::compile::<0, _>([(0, SOURCE)].as_slice(), &mut out)
            .expect("examples should always compile");

        assert_eq!(
            str::from_utf8(out.as_slice()),
            Ok("true\nfalse\ntrue\nfalse\n")
        );
    }

    #[test]
    fn three_or_four_register() {
        let mut out = vec![];

        let _ = apart::compile::<32, _>([(0, SOURCE)].as_slice(), &mut out)
            .expect("examples should always compile");

        assert_eq!(
            str::from_utf8(out.as_slice()),
            Ok("true\nfalse\ntrue\nfalse\n")
        );
    }
}
