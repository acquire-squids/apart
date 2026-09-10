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

#[cfg(test)]
mod sum_equality_0 {
    const SOURCE: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/lang/tests/",
        "sum_equality_0.txt"
    ));

    #[test]
    fn sum_equality_0() {
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
            )]) if expected == "bool" && got == "unit"
        );
    }
}

#[cfg(test)]
mod sum_equality_1 {
    const SOURCE: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/lang/tests/",
        "sum_equality_1.txt"
    ));

    #[test]
    fn sum_equality_1() {
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
            )]) if expected == "bool" && got == "unit"
        );
    }
}

#[cfg(test)]
mod path_explicit_self {
    const SOURCE: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/lang/tests/",
        "path_explicit_self.txt"
    ));

    #[test]
    fn path_explicit_self() {
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
                apart::NameResolveError::PathDoesNotExist,
            )])
        );
    }
}

#[cfg(test)]
mod path_incorrect_argument {
    const SOURCE: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/lang/tests/",
        "path_incorrect_argument.txt"
    ));

    #[test]
    fn path_incorrect_argument() {
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
                }
            )]) if expected == "i64" && got == "funky(T) -> T"
        );
    }
}

#[cfg(test)]
mod path_invalid {
    const SOURCE: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/lang/tests/",
        "path_invalid.txt"
    ));

    #[test]
    fn path_invalid() {
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
                apart::NameResolveError::InvalidPath
            )])
        );
    }
}

#[cfg(test)]
mod path_nonexistent {
    const SOURCE: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/lang/tests/",
        "path_nonexistent.txt"
    ));

    #[test]
    fn path_nonexistent() {
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
                apart::NameResolveError::PathCannotAssociate,
            )])
        );
    }
}

#[cfg(test)]
mod teach_generic_confusion {
    const SOURCE: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/lang/tests/",
        "teach_generic_confusion.txt"
    ));

    #[test]
    fn teach_generic_confusion() {
        let mut out = vec![];

        let compiled = apart::compile::<0, _>([(0, SOURCE)].as_slice(), &mut out);

        let errors = compiled.as_ref().map_err(|errors| {
            errors
                .iter()
                .map(reporting::Spanned::kind)
                .collect::<Vec<_>>()
        });

        let errors = errors.as_ref().map_err(std::vec::Vec::as_slice);

        std::assert_matches!(errors, Err([apart::Error::TypeCheck(apart::TypeCheckError::TypeMismatch {
            expected,
            got,
        })]) if expected == "Wrapper[i64]" && got == "Wrapper[unit]");
    }
}

#[cfg(test)]
mod teach_generic_confusion_method_access {
    const SOURCE: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/lang/tests/",
        "teach_generic_confusion_method_access.txt"
    ));

    #[test]
    fn teach_generic_confusion_method_access() {
        let mut out = vec![];

        let compiled = apart::compile::<0, _>([(0, SOURCE)].as_slice(), &mut out);

        let errors = compiled.as_ref().map_err(|errors| {
            errors
                .iter()
                .map(reporting::Spanned::kind)
                .collect::<Vec<_>>()
        });

        let errors = errors.as_ref().map_err(std::vec::Vec::as_slice);

        std::assert_matches!(errors, Err([apart::Error::TypeCheck(apart::TypeCheckError::TypeMismatch {
            expected,
            got,
        })]) if expected == "Wrapper[i64]" && got == "Wrapper[unit]");
    }
}

#[cfg(test)]
mod callee_is_access_error {
    const SOURCE: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/lang/tests/",
        "callee_is_access_error.txt"
    ));

    #[test]
    fn callee_is_access_error() {
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
                apart::TypeCheckError::FnFieldAsMethod
            )])
        );
    }
}
