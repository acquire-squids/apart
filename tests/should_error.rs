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

        let errors = compiled.as_ref().map_err(|errors| {
            errors
                .iter()
                .map(reporting::Spanned::kind)
                .collect::<Vec<_>>()
        });

        let errors = errors.as_ref().map_err(std::vec::Vec::as_slice);

        std::assert_matches!(
            errors,
            Err([apart::Error::NameResolve(
                apart::NameResolveError::InvalidAssignTarget
            )])
        );
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

        let errors = compiled.as_ref().map_err(|errors| {
            errors
                .iter()
                .map(reporting::Spanned::kind)
                .collect::<Vec<_>>()
        });

        let errors = errors.as_ref().map_err(std::vec::Vec::as_slice);

        std::assert_matches!(
            errors,
            Err([apart::Error::TypeCheck(
                apart::TypeCheckError::TypeMismatch {
                    expected,
                    got,
                },
            )]) if expected == "T" && got == "U"
        );
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

        let errors = compiled.as_ref().map_err(|errors| {
            errors
                .iter()
                .map(reporting::Spanned::kind)
                .collect::<Vec<_>>()
        });

        let errors = errors.as_ref().map_err(std::vec::Vec::as_slice);

        std::assert_matches!(
            errors,
            Err([apart::Error::TypeCheck(
                apart::TypeCheckError::TypeMismatch {
                    expected,
                    got,
                },
            )]) if expected == "unit" && got == "bool"
        );
    }
}
