#[cfg(test)]
mod product {
    const SOURCE: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/lang/tests/",
        "product.txt"
    ));

    #[test]
    fn product() {
        let mut out = vec![];

        let _ = apart::compile::<0, _>([(0, SOURCE)].as_slice(), &mut out)
            .expect("examples should always compile");

        assert_eq!(str::from_utf8(out.as_slice()), Ok(""));
    }

    #[test]
    fn product_register() {
        let mut out = vec![];

        let _ = apart::compile::<32, _>([(0, SOURCE)].as_slice(), &mut out)
            .expect("examples should always compile");

        assert_eq!(str::from_utf8(out.as_slice()), Ok(""));
    }
}

#[cfg(test)]
mod product_identity {
    const SOURCE: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/lang/tests/",
        "product_identity.txt"
    ));

    #[test]
    fn product_identity() {
        let mut out = vec![];

        let _ = apart::compile::<0, _>([(0, SOURCE)].as_slice(), &mut out)
            .expect("examples should always compile");

        assert_eq!(str::from_utf8(out.as_slice()), Ok("3.14\n"));
    }

    #[test]
    fn product_identity_register() {
        let mut out = vec![];

        let _ = apart::compile::<32, _>([(0, SOURCE)].as_slice(), &mut out)
            .expect("examples should always compile");

        assert_eq!(str::from_utf8(out.as_slice()), Ok("3.14\n"));
    }
}

#[cfg(test)]
mod product_rearrange {
    const SOURCE: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/lang/tests/",
        "product_rearrange.txt"
    ));

    #[test]
    fn product_rearrange() {
        let mut out = vec![];

        let _ = apart::compile::<0, _>([(0, SOURCE)].as_slice(), &mut out)
            .expect("examples should always compile");

        assert_eq!(str::from_utf8(out.as_slice()), Ok("1.0\n2.0\n3.0\n"));
    }

    #[test]
    fn product_rearrange_register() {
        let mut out = vec![];

        let _ = apart::compile::<32, _>([(0, SOURCE)].as_slice(), &mut out)
            .expect("examples should always compile");

        assert_eq!(str::from_utf8(out.as_slice()), Ok("1.0\n2.0\n3.0\n"));
    }
}

#[cfg(test)]
mod product_ssa_if_0 {
    const SOURCE: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/lang/tests/",
        "product_ssa_if_0.txt"
    ));

    #[test]
    fn product_ssa_if_0() {
        let mut out = vec![];

        let _ = apart::compile::<0, _>([(0, SOURCE)].as_slice(), &mut out)
            .expect("examples should always compile");

        assert_eq!(str::from_utf8(out.as_slice()), Ok("21\n"));
    }

    #[test]
    fn product_ssa_if_0_register() {
        let mut out = vec![];

        let _ = apart::compile::<32, _>([(0, SOURCE)].as_slice(), &mut out)
            .expect("examples should always compile");

        assert_eq!(str::from_utf8(out.as_slice()), Ok("21\n"));
    }
}

#[cfg(test)]
mod product_ssa_if_1 {
    const SOURCE: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/lang/tests/",
        "product_ssa_if_1.txt"
    ));

    #[test]
    fn product_ssa_if_1() {
        let mut out = vec![];

        let _ = apart::compile::<0, _>([(0, SOURCE)].as_slice(), &mut out)
            .expect("examples should always compile");

        assert_eq!(str::from_utf8(out.as_slice()), Ok("19\n"));
    }

    #[test]
    fn product_ssa_if_1_register() {
        let mut out = vec![];

        let _ = apart::compile::<32, _>([(0, SOURCE)].as_slice(), &mut out)
            .expect("examples should always compile");

        assert_eq!(str::from_utf8(out.as_slice()), Ok("19\n"));
    }
}

#[cfg(test)]
mod product_ssa_if_2 {
    const SOURCE: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/lang/tests/",
        "product_ssa_if_2.txt"
    ));

    #[test]
    fn product_ssa_if_2() {
        let mut out = vec![];

        let _ = apart::compile::<0, _>([(0, SOURCE)].as_slice(), &mut out)
            .expect("examples should always compile");

        assert_eq!(str::from_utf8(out.as_slice()), Ok("38\n"));
    }

    #[test]
    fn product_ssa_if_2_register() {
        let mut out = vec![];

        let _ = apart::compile::<32, _>([(0, SOURCE)].as_slice(), &mut out)
            .expect("examples should always compile");

        assert_eq!(str::from_utf8(out.as_slice()), Ok("38\n"));
    }
}

#[cfg(test)]
mod product_return_type {
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
}

#[cfg(test)]
mod product_fn_parameter {
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
}

#[cfg(test)]
mod product_three_or_four {
    const SOURCE: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/lang/tests/",
        "product_three_or_four.txt"
    ));

    #[test]
    fn product_three_or_four() {
        let mut out = vec![];

        let _ = apart::compile::<0, _>([(0, SOURCE)].as_slice(), &mut out)
            .expect("examples should always compile");

        assert_eq!(str::from_utf8(out.as_slice()), Ok("3\n"));
    }

    #[test]
    fn product_three_or_four_register() {
        let mut out = vec![];

        let _ = apart::compile::<32, _>([(0, SOURCE)].as_slice(), &mut out)
            .expect("examples should always compile");

        assert_eq!(str::from_utf8(out.as_slice()), Ok("3\n"));
    }
}

#[cfg(test)]
mod product_equality {
    const SOURCE: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/lang/tests/",
        "product_equality.txt"
    ));

    #[test]
    fn product_equality() {
        let mut out = vec![];

        let _ = apart::compile::<0, _>([(0, SOURCE)].as_slice(), &mut out)
            .expect("examples should always compile");

        assert_eq!(str::from_utf8(out.as_slice()), Ok("true\nfalse\n"));
    }

    #[test]
    fn product_equality_register() {
        let mut out = vec![];

        let _ = apart::compile::<32, _>([(0, SOURCE)].as_slice(), &mut out)
            .expect("examples should always compile");

        assert_eq!(str::from_utf8(out.as_slice()), Ok("true\nfalse\n"));
    }
}
