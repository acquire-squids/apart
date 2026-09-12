mod evaluation_output {
    apart::test_evaluation_output!(
        iterative_fibonacci,
        "iterative_fibonacci.txt",
        "7540113804746346429\n"
    );

    apart::test_evaluation_output!(recursive_fibonacci, "recursive_fibonacci.txt", "6765\n");

    apart::test_evaluation_output!(you_stupid, "you_stupid.txt", "true\nfalse\n");

    apart::test_evaluation_output!(zoo, "zoo.txt", "100002\n");

    apart::test_evaluation_output!(identity, "identity.txt", "21\n");

    apart::test_evaluation_output!(nested_generics, "nested_generics.txt", "3.14\n");

    apart::test_evaluation_output!(access_lhs_is_call, "access_lhs_is_call.txt", "true\n");

    apart::test_evaluation_output!(callee_is_access, "callee_is_access.txt", "true\n");

    apart::test_evaluation_output!(callee_is_call, "callee_is_call.txt", "true\n");

    apart::test_evaluation_output!(and, "and.txt", "true\nfalse\n");

    apart::test_evaluation_output!(and_and_and_and_and, "and_and_and_and_and.txt", "true\n");

    apart::test_evaluation_output!(and_and_or, "and_and_or.txt", "true\n");

    apart::test_evaluation_output!(empty, "empty.txt", "");

    apart::test_evaluation_output!(fn_parameter, "fn_parameter.txt", "21\n");

    apart::test_evaluation_output!(r#if, "if.txt", "21.0\n");

    apart::test_evaluation_output!(if_else_if_else, "if_else_if_else.txt", "3\n2\n1\n1\n");

    apart::test_evaluation_output!(or, "or.txt", "true\nfalse\n");

    apart::test_evaluation_output!(or_and_or_and_or, "or_and_or_and_or.txt", "true\n");

    apart::test_evaluation_output!(equality, "equality.txt", "true\nfalse\ntrue\nfalse\n");

    apart::test_evaluation_output!(r#while, "while.txt", "{}\n{}\n{}\n");

    apart::test_evaluation_output!(noppers, "noppers.txt", "80\n");

    apart::test_evaluation_output!(r#mod, "mod.txt", "97\n");

    apart::test_evaluation_output!(module_generics, "module_generics.txt", "3.0\n");

    apart::test_evaluation_output!(root_path, "root_path.txt", "97\n");

    apart::test_evaluation_output!(product, "product.txt", "");

    apart::test_evaluation_output!(product_identity, "product_identity.txt", "3.14\n");

    apart::test_evaluation_output!(
        product_rearrange,
        "product_rearrange.txt",
        "1.0\n2.0\n3.0\n"
    );

    apart::test_evaluation_output!(product_ssa_if_0, "product_ssa_if_0.txt", "21\n");

    apart::test_evaluation_output!(product_ssa_if_1, "product_ssa_if_1.txt", "19\n");

    apart::test_evaluation_output!(product_ssa_if_2, "product_ssa_if_2.txt", "38\n");

    apart::test_evaluation_output!(product_return_type, "product_return_type.txt", "{}\n");

    apart::test_evaluation_output!(product_fn_parameter, "product_fn_parameter.txt", "{}\n");

    apart::test_evaluation_output!(product_three_or_four, "product_three_or_four.txt", "3\n");

    apart::test_evaluation_output!(product_equality, "product_equality.txt", "true\nfalse\n");

    apart::test_evaluation_output!(ssa_if_0, "ssa_if_0.txt", "21\n");

    apart::test_evaluation_output!(ssa_if_1, "ssa_if_1.txt", "19\n");

    apart::test_evaluation_output!(ssa_if_2, "ssa_if_2.txt", "38\n");

    apart::test_evaluation_output!(three_or_four, "three_or_four.txt", "3\n");

    apart::test_evaluation_output!(sum, "sum.txt", "false\ntrue\n");

    apart::test_evaluation_output!(sum_equality_2, "sum_equality_2.txt", "false\n");

    apart::test_evaluation_output!(sum_equality_3, "sum_equality_3.txt", "false\n");

    apart::test_evaluation_output!(teach, "teach.txt", "97\n",);

    apart::test_evaluation_output!(teach_method_access, "teach_method_access.txt", "97\n",);
}

mod compilation_error {
    apart::test_compilation_errors!(
        invalid_assign_target,
        "invalid_assign_target.txt",
        [apart::Error::NameResolve(
            apart::NameResolveError::InvalidAssignTarget
        )]
    );

    apart::test_compilation_errors!(
        generic_equality,
        "generic_equality.txt",
        [apart::Error::TypeCheck(apart::TypeCheckError::TypeMismatch {
            expected, got
        })] if expected == "T" && got == "U"
    );

    apart::test_compilation_errors!(
        callee_is_call_invalid,
        "callee_is_call_invalid.txt",
        [apart::Error::TypeCheck(apart::TypeCheckError::TypeMismatch {
            expected, got
        })] if expected == "unit" && got == "bool"
    );

    apart::test_compilation_errors!(
        sum_equality_0,
        "sum_equality_0.txt",
        [apart::Error::TypeCheck(apart::TypeCheckError::TypeMismatch {
            expected, got
        })] if expected == "bool" && got == "unit"
    );

    apart::test_compilation_errors!(
        sum_equality_1,
        "sum_equality_1.txt",
        [apart::Error::TypeCheck(apart::TypeCheckError::TypeMismatch {
            expected, got
        })] if expected == "bool" && got == "unit"
    );

    apart::test_compilation_errors!(
        path_explicit_self,
        "path_explicit_self.txt",
        [apart::Error::NameResolve(
            apart::NameResolveError::PathDoesNotExist
        )]
    );

    apart::test_compilation_errors!(
        path_incorrect_argument,
        "path_incorrect_argument.txt",
        [apart::Error::TypeCheck(apart::TypeCheckError::TypeMismatch {
            expected, got
        })] if expected == "i64" && got == "funky(T) -> T"
    );

    apart::test_compilation_errors!(
        path_invalid,
        "path_invalid.txt",
        [apart::Error::NameResolve(
            apart::NameResolveError::InvalidPath
        )]
    );

    apart::test_compilation_errors!(
        path_nonexistent,
        "path_nonexistent.txt",
        [apart::Error::NameResolve(
            apart::NameResolveError::PathCannotAssociate
        )]
    );

    apart::test_compilation_errors!(
        teach_generic_confusion,
        "teach_generic_confusion.txt",
        [apart::Error::TypeCheck(apart::TypeCheckError::TypeMismatch {
            expected, got
        })] if expected == "Wrapper[i64]" && got == "Wrapper[unit]"
    );

    apart::test_compilation_errors!(
        teach_generic_confusion_method_access,
        "teach_generic_confusion_method_access.txt",
        [apart::Error::TypeCheck(apart::TypeCheckError::TypeMismatch {
            expected, got
        })] if expected == "Wrapper[i64]" && got == "Wrapper[unit]"
    );

    apart::test_compilation_errors!(
        callee_is_access_error,
        "callee_is_access_error.txt",
        [apart::Error::TypeCheck(
            apart::TypeCheckError::FnFieldAsMethod
        )]
    );
}
