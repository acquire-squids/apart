#[cfg(test)]
mod iterative_fibonacci {
    const SOURCE: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/lang/tests/",
        "iterative_fibonacci.txt"
    ));

    #[test]
    fn iterative_fibonacci() {
        let mut out = vec![];

        let _ = apart::compile::<0, _>([(0, SOURCE)].as_slice(), &mut out)
            .expect("examples should always compile");

        assert_eq!(str::from_utf8(out.as_slice()), Ok("7540113804746346429\n"));
    }

    #[test]
    fn iterative_fibonacci_register() {
        let mut out = vec![];

        let _ = apart::compile::<32, _>([(0, SOURCE)].as_slice(), &mut out)
            .expect("examples should always compile");

        assert_eq!(str::from_utf8(out.as_slice()), Ok("7540113804746346429\n"));
    }
}

#[cfg(test)]
mod recursive_fibonacci {
    const SOURCE: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/lang/tests/",
        "recursive_fibonacci.txt"
    ));

    #[test]
    fn recursive_fibonacci() {
        let mut out = vec![];

        let _ = apart::compile::<0, _>([(0, SOURCE)].as_slice(), &mut out)
            .expect("examples should always compile");

        assert_eq!(str::from_utf8(out.as_slice()), Ok("6765\n"));
    }

    #[test]
    fn recursive_fibonacci_register() {
        let mut out = vec![];

        let _ = apart::compile::<32, _>([(0, SOURCE)].as_slice(), &mut out)
            .expect("examples should always compile");

        assert_eq!(str::from_utf8(out.as_slice()), Ok("6765\n"));
    }
}

#[cfg(test)]
mod you_stupid {
    const SOURCE: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/lang/tests/",
        "you_stupid.txt"
    ));

    #[test]
    fn you_stupid() {
        let mut out = vec![];

        let _ = apart::compile::<0, _>([(0, SOURCE)].as_slice(), &mut out)
            .expect("examples should always compile");

        assert_eq!(str::from_utf8(out.as_slice()), Ok("true\nfalse\n"));
    }

    #[test]
    fn you_stupid_register() {
        let mut out = vec![];

        let _ = apart::compile::<32, _>([(0, SOURCE)].as_slice(), &mut out)
            .expect("examples should always compile");

        assert_eq!(str::from_utf8(out.as_slice()), Ok("true\nfalse\n"));
    }
}
