#[cfg(test)]
mod invalid_assign_target {
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
}

#[cfg(test)]
mod generic_equality {
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
}

#[cfg(test)]
mod callee_is_call_invalid {
    const SOURCE: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/lang/tests/",
        "callee_is_call_invalid.txt"
    ));

    #[test]
    fn callee_is_call_invalid() {
        let mut out = vec![];

        let compiled = apart::compile::<0, _>([(0, SOURCE)].as_slice(), &mut out);

        std::assert_matches!(compiled, Err(_));
    }
}
