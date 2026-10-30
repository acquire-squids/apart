macro_rules! output_tests {
    (
        $module_name:ident, $test_macro:ident ;
    ) => {
        mod $module_name {
            apart::$test_macro!(access_lhs_is_call, "access_lhs_is_call.txt", Ok("true\n"));

            apart::$test_macro!(and, "and.txt", Ok("true\nfalse\n"));

            apart::$test_macro!(and_and_and_and_and, "and_and_and_and_and.txt", Ok("true\n"));

            apart::$test_macro!(and_and_or, "and_and_or.txt", Ok("true\n"));

            apart::$test_macro!(
                assign_to_fn_parameter,
                "assign_to_fn_parameter.txt",
                Ok("{}\n")
            );

            apart::$test_macro!(callee_is_access, "callee_is_access.txt", Ok("true\n"));

            apart::$test_macro!(callee_is_call, "callee_is_call.txt", Ok("true\n"));

            apart::$test_macro!(directive, "directive.txt", Ok("vm\n"));

            apart::$test_macro!(empty, "empty.txt", Ok(""));

            apart::$test_macro!(equality, "equality.txt", Ok("true\nfalse\ntrue\nfalse\n"));

            apart::$test_macro!(f32, "f32.txt", Ok("123.456\n"));

            apart::$test_macro!(forward_alias, "forward_alias.txt", Ok("true\n"));

            apart::$test_macro!(fn_parameter, "fn_parameter.txt", Ok("21\n"));

            apart::$test_macro!(hello_bingus, "hello_bingus.txt", Ok("Hello, Bingus!\n"));

            apart::$test_macro!(hello_world, "hello_world.txt", Ok("Hello, world!\n"));

            apart::$test_macro!(identity, "identity.txt", Ok("21\n"));

            apart::$test_macro!(if_as_binary_lhs, "if_as_binary_lhs.txt", Ok("-3\n"),);

            apart::$test_macro!(if_as_call_argument, "if_as_call_argument.txt", Ok("21\n"),);

            apart::$test_macro!(
                if_as_method_access,
                "if_as_method_access.txt",
                Ok("true\nfalse\n"),
            );

            apart::$test_macro!(
                if_condition_is_access,
                "if_condition_is_access.txt",
                Ok("1\n"),
            );

            apart::$test_macro!(if_else_if_else, "if_else_if_else.txt", Ok("3\n2\n1\n1\n"));

            apart::$test_macro!(is_some, "is_some.txt", Ok("true\nfalse\n"),);

            apart::$test_macro!(
                iterative_fibonacci,
                "iterative_fibonacci.txt",
                Ok("7540113804746346429\n")
            );

            apart::$test_macro!(
                method_specialization,
                "method_specialization.txt",
                Ok("1\n0\n2\n3\n4\n"),
            );

            apart::$test_macro!(module_generics, "module_generics.txt", Ok("3.0\n"));

            apart::$test_macro!(nested_generics, "nested_generics.txt", Ok("3.14\n"));

            apart::$test_macro!(nested_options, "nested_options.txt", Ok(""),);

            apart::$test_macro!(no_ranges, "no_ranges.txt", Ok("true\ntrue\ntrue\nfalse\n"),);

            apart::$test_macro!(noppers, "noppers.txt", Ok("80\n"));

            apart::$test_macro!(
                option_expect,
                "option_expect.txt",
                Err((
                    "97\nb is not fine!\nVM panicked\n",
                    apart::vm::Interrupt::Panic
                ))
            );

            apart::$test_macro!(
                option_unwrap,
                "option_unwrap.txt",
                Err((
                    "97\ntried to unwrap a None value\nVM panicked\n",
                    apart::vm::Interrupt::Panic
                ))
            );

            apart::$test_macro!(or, "or.txt", Ok("true\nfalse\n"));

            apart::$test_macro!(or_and_or_and_or, "or_and_or_and_or.txt", Ok("true\n"));

            apart::$test_macro!(path_super, "path_super.txt", Ok("97\n"));

            apart::$test_macro!(
                pattern_field_elision,
                "pattern_field_elision.txt",
                Ok("true\n"),
            );

            apart::$test_macro!(product, "product.txt", Ok(""));

            apart::$test_macro!(
                product_elide_name_field,
                "product_elide_name_field.txt",
                Ok("-3\n")
            );

            apart::$test_macro!(
                product_equality,
                "product_equality.txt",
                Ok("true\nfalse\n")
            );

            apart::$test_macro!(product_fn_parameter, "product_fn_parameter.txt", Ok("{}\n"));

            apart::$test_macro!(product_identity, "product_identity.txt", Ok("3.14\n"));

            apart::$test_macro!(
                product_rearrange,
                "product_rearrange.txt",
                Ok("1.0\n2.0\n3.0\n")
            );

            apart::$test_macro!(product_return_type, "product_return_type.txt", Ok("{}\n"));

            apart::$test_macro!(product_self, "product_self.txt", Ok("38\n"));

            apart::$test_macro!(product_ssa_if_0, "product_ssa_if_0.txt", Ok("21\n"));

            apart::$test_macro!(product_ssa_if_1, "product_ssa_if_1.txt", Ok("19\n"));

            apart::$test_macro!(product_ssa_if_2, "product_ssa_if_2.txt", Ok("38\n"));

            apart::$test_macro!(
                product_three_or_four,
                "product_three_or_four.txt",
                Ok("3\n")
            );

            apart::$test_macro!(r#if, "if.txt", Ok("21.0\n"));

            apart::$test_macro!(r#mod, "mod.txt", Ok("97\n"));

            apart::$test_macro!(r#while, "while.txt", Ok("{}\n{}\n{}\n"));

            apart::$test_macro!(recursive_fibonacci, "recursive_fibonacci.txt", Ok("6765\n"));

            apart::$test_macro!(
                result_expect,
                "result_expect.txt",
                Err((
                    "97\nb is not fine!\nVM panicked\n",
                    apart::vm::Interrupt::Panic
                ))
            );

            apart::$test_macro!(
                result_unwrap,
                "result_unwrap.txt",
                Err((
                    "97\ntried to unwrap an Err value\nVM panicked\n",
                    apart::vm::Interrupt::Panic
                ))
            );

            apart::$test_macro!(root_path, "root_path.txt", Ok("97\n"));

            apart::$test_macro!(speed_test, "speed_test.txt", Ok(""),);

            apart::$test_macro!(
                sibling_module_inner_inner,
                "sibling_module_inner_inner.txt",
                Ok(""),
            );

            apart::$test_macro!(signed_min, "signed_min.txt", Ok("-128\n"),);

            apart::$test_macro!(some_some_none, "some_some_none.txt", Ok("true\n21\n"),);

            apart::$test_macro!(ssa_if_0, "ssa_if_0.txt", Ok("21\n"));

            apart::$test_macro!(ssa_if_1, "ssa_if_1.txt", Ok("19\n"));

            apart::$test_macro!(ssa_if_2, "ssa_if_2.txt", Ok("38\n"));

            apart::$test_macro!(sum, "sum.txt", Ok("false\ntrue\n"));

            apart::$test_macro!(sum_equality_2, "sum_equality_2.txt", Ok("false\n"));

            apart::$test_macro!(sum_equality_3, "sum_equality_3.txt", Ok("false\n"));

            apart::$test_macro!(teach, "teach.txt", Ok("97\n"),);

            apart::$test_macro!(
                teach_generic_clarity,
                "teach_generic_clarity.txt",
                Ok("97\n{}\n"),
            );

            apart::$test_macro!(
                teach_generic_clarity_method_access,
                "teach_generic_clarity_method_access.txt",
                Ok("97\n{}\n"),
            );

            apart::$test_macro!(teach_method_access, "teach_method_access.txt", Ok("97\n"),);

            apart::$test_macro!(three_or_four, "three_or_four.txt", Ok("3\n"));

            apart::$test_macro!(type_annotation, "type_annotation.txt", Ok("21\n"),);

            apart::$test_macro!(
                vec,
                "vec.txt",
                Ok("[0,1,2,]\nPOP!\n2\n[0,1,]\nEXTEND!\n[0,1,0,1,]\nTRUNCATE!\n[]\nPOP!\n"),
            );

            apart::$test_macro!(
                we_have_if_at_home,
                "we_have_if_at_home.txt",
                Ok("true\nfalse\n"),
            );

            apart::$test_macro!(you_stupid, "you_stupid.txt", Ok("true\nfalse\n"));

            apart::$test_macro!(zoo, "zoo.txt", Ok("100002\n"));
        }
    };
}

output_tests!(
    vm_output, test_vm_output ;
);

mod compilation_error {
    apart::test_compilation_errors!(
        alias_self_recursion,
        "alias_self_recursion.txt",
        [apart::Error::NameResolve(
            apart::NameResolveError::RecursiveAlias
        )]
    );

    apart::test_compilation_errors!(
        alias_mutual_recursion,
        "alias_mutual_recursion.txt",
        [apart::Error::NameResolve(
            apart::NameResolveError::RecursiveAlias
        )]
    );

    apart::test_compilation_errors!(
        assign_to_fn,
        "assign_to_fn.txt",
        [apart::Error::NameResolve(
            apart::NameResolveError::AssignmentTargetIsFunction
        )]
    );

    apart::test_compilation_errors!(
        assign_to_native_fn,
        "assign_to_native_fn.txt",
        [apart::Error::NameResolve(
            apart::NameResolveError::AssignmentTargetIsFunction
        )]
    );

    apart::test_compilation_errors!(
        bad_token_tree_0,
        "bad_token_tree_0.txt",
        [apart::Error::Parse(
            apart::ParseError::BinaryWithoutRightOperand
        )]
    );

    apart::test_compilation_errors!(
        bad_token_tree_1,
        "bad_token_tree_1.txt",
        [apart::Error::Parse(apart::ParseError::MatchWithoutValue)]
    );

    apart::test_compilation_errors!(
        bad_token_tree_2,
        "bad_token_tree_2.txt",
        [apart::Error::Parse(apart::ParseError::IfWithoutCondition)]
    );

    apart::test_compilation_errors!(
        bad_token_tree_3,
        "bad_token_tree_3.txt",
        [apart::Error::Parse(apart::ParseError::LetWithoutName)]
    );

    apart::test_compilation_errors!(
        bad_token_tree_4,
        "bad_token_tree_4.txt",
        [apart::Error::Parse(
            apart::ParseError::WhileWithoutCondition
        )]
    );

    apart::test_compilation_errors!(
        bad_token_tree_5,
        "bad_token_tree_5.txt",
        [apart::Error::Parse(apart::ParseError::UnaryWithoutOperand)]
    );

    apart::test_compilation_errors!(
        bad_token_tree_6,
        "bad_token_tree_6.txt",
        [apart::Error::Parse(apart::ParseError::ReturnWithoutValue)]
    );

    apart::test_compilation_errors!(
        callee_is_access_error,
        "callee_is_access_error.txt",
        [apart::Error::TypeCheck(
            apart::TypeCheckError::FnFieldAsMethod
        )]
    );

    apart::test_compilation_errors!(
        callee_is_call_invalid,
        "callee_is_call_invalid.txt",
        [apart::Error::TypeCheck(apart::TypeCheckError::TypeMismatch {
            expected, got
        })] if expected == "unit" && got == "bool"
    );

    apart::test_compilation_errors!(
        directive_never,
        "directive_never.txt",
        [apart::Error::NameResolve(
            apart::NameResolveError::PathDoesNotExist
        )]
    );

    apart::test_compilation_errors!(
        f32_add_f64,
        "f32_add_f64.txt",
        [
            apart::Error::TypeCheck(apart::TypeCheckError::TypeMismatch {
                expected: a_expected,
                got: a_got
            }),
            apart::Error::TypeCheck(apart::TypeCheckError::TypeMismatch {
                expected: b_expected,
                got: b_got
            })
        ] if a_expected == "f32" && a_got == "f64" && b_expected == "f64" && b_got == "f32"
    );

    apart::test_compilation_errors!(
        forward_alias_generic_mismatch,
        "forward_alias_generic_mismatch.txt",
        [
            apart::Error::TypeCheck(apart::TypeCheckError::GenericCountMismatch {
                expected: 2,
                got: 0
            }),
            apart::Error::TypeCheck(apart::TypeCheckError::GenericCountMismatch {
                expected: 2,
                got: 0
            })
        ]
    );

    apart::test_compilation_errors!(
        generic_count_mismatch,
        "generic_count_mismatch.txt",
        [apart::Error::TypeCheck(
            apart::TypeCheckError::GenericCountMismatch {
                expected: 2,
                got: 1
            }
        )]
    );

    apart::test_compilation_errors!(
        generic_count_mismatch_module,
        "generic_count_mismatch_module.txt",
        [apart::Error::TypeCheck(
            apart::TypeCheckError::GenericCountMismatch {
                expected: 2,
                got: 1
            }
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
        generics_unchecked,
        "generics_unchecked.txt",
        [apart::Error::TypeCheck(
            apart::TypeCheckError::TypeMismatch {
                expected, got,
            }
        )] if expected == "Either[T, bool]" && got == "Either[T, i64]"
    );

    apart::test_compilation_errors!(
        if_without_else,
        "if_without_else.txt",
        [apart::Error::TypeCheck(apart::TypeCheckError::TypeMismatch {
            expected, got
        })] if expected == "unit" && got == "i64"
    );

    apart::test_compilation_errors!(
        inner_module_private,
        "inner_module_private.txt",
        [apart::Error::NameResolve(
            apart::NameResolveError::PathIsPrivate
        )]
    );

    apart::test_compilation_errors!(
        inner_module_product_private,
        "inner_module_product_private.txt",
        [
            apart::Error::NameResolve(apart::NameResolveError::PathIsPrivate),
            apart::Error::NameResolve(apart::NameResolveError::PathIsPrivate)
        ]
    );

    apart::test_compilation_errors!(
        invalid_assign_target,
        "invalid_assign_target.txt",
        [apart::Error::NameResolve(
            apart::NameResolveError::InvalidAssignTarget
        )]
    );

    apart::test_compilation_errors!(
        native_method_without_target,
        "native_method_without_target.txt",
        [apart::Error::NameResolve(
            apart::NameResolveError::NameNotDeclared
        )]
    );

    apart::test_compilation_errors!(
        negative_unsigned,
        "negative_unsigned.txt",
        [apart::Error::TypeCheck(
            apart::TypeCheckError::CannotNegate(ty)
        )] if ty == "u32"
    );

    apart::test_compilation_errors!(
        never_type,
        "never_type.txt",
        [apart::Error::TypeCheck(
            apart::TypeCheckError::TypeMismatch {
                expected: a_expected, got: a_got
            }
        ), apart::Error::TypeCheck(
            apart::TypeCheckError::TypeMismatch {
                expected: b_expected, got: b_got
            }
        ), apart::Error::TypeCheck(
            apart::TypeCheckError::TypeMismatch {
                expected: c_expected, got: c_got
            }
        )] if a_expected == "never"
            && a_got == "i64"
            && b_expected == "never"
            && b_got == "f64"
            && c_expected == "never"
            && c_got == "f64"
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
        path_more_private_than_use,
        "path_more_private_than_use.txt",
        [
            apart::Error::NameResolve(apart::NameResolveError::PathMorePrivateThanUse),
            apart::Error::NameResolve(apart::NameResolveError::PathMorePrivateThanUse)
        ]
    );

    apart::test_compilation_errors!(
        path_nonexistent,
        "path_nonexistent.txt",
        [apart::Error::NameResolve(
            apart::NameResolveError::PathCannotAssociate
        )]
    );

    apart::test_compilation_errors!(
        path_super_invalid,
        "path_super_invalid.txt",
        [apart::Error::NameResolve(
            apart::NameResolveError::SuperAtRoot
        )]
    );

    apart::test_compilation_errors!(
        print_access_assign,
        "print_access_assign.txt",
        [
            apart::Error::TypeCheck(apart::TypeCheckError::TypeMismatch {
                expected, got,
            }),
        ] if expected == "u32" && got == "unit"
    );

    apart::test_compilation_errors!(
        print_assign,
        "print_assign.txt",
        [
            apart::Error::TypeCheck(apart::TypeCheckError::TypeMismatch {
                expected, got,
            }),
        ] if expected == "u32" && got == "unit"
    );

    apart::test_compilation_errors!(
        private_field,
        "private_field.txt",
        [
            apart::Error::TypeCheck(apart::TypeCheckError::PrivateField),
            apart::Error::TypeCheck(apart::TypeCheckError::PrivateField),
            apart::Error::TypeCheck(apart::TypeCheckError::PrivateField)
        ]
    );

    apart::test_compilation_errors!(
        private_function,
        "private_function.txt",
        [apart::Error::NameResolve(
            apart::NameResolveError::PathIsPrivate
        )]
    );

    apart::test_compilation_errors!(
        private_method,
        "private_method.txt",
        [apart::Error::TypeCheck(
            apart::TypeCheckError::PrivateMethod
        )]
    );

    apart::test_compilation_errors!(
        root_path_invalid,
        "root_path_invalid.txt",
        [apart::Error::NameResolve(
            apart::NameResolveError::RootDeeperThanPathStart
        )]
    );

    apart::test_compilation_errors!(
        sibling_module_private,
        "sibling_module_private.txt",
        [apart::Error::NameResolve(
            apart::NameResolveError::PathIsPrivate
        )]
    );

    apart::test_compilation_errors!(
        sibling_module_inner_inner_self,
        "sibling_module_inner_inner_self.txt",
        [apart::Error::NameResolve(
            apart::NameResolveError::PathDoesNotExist
        )]
    );

    apart::test_compilation_errors!(
        sibling_module_product_private,
        "sibling_module_product_private.txt",
        [
            apart::Error::NameResolve(apart::NameResolveError::PathIsPrivate,),
            apart::Error::NameResolve(apart::NameResolveError::PathIsPrivate,)
        ]
    );

    apart::test_compilation_errors!(
        signed_equals_unsigned,
        "signed_equals_unsigned.txt",
        [apart::Error::TypeCheck(apart::TypeCheckError::TypeMismatch {
            expected, got
        })] if expected == "i32" && got == "u32"
    );

    apart::test_compilation_errors!(
        some_without_option,
        "some_without_option.txt",
        [apart::Error::NameResolve(
            apart::NameResolveError::NameNotDeclared
        )]
    );

    apart::test_compilation_errors!(
        sum_equality_0,
        "sum_equality_0.txt",
        [apart::Error::TypeCheck(apart::TypeCheckError::TypeMismatch {
            expected, got
        })] if expected == "Option[Result[bool, i64]]" && got == "Option[Result[unit, i64]]"
    );

    apart::test_compilation_errors!(
        sum_equality_1,
        "sum_equality_1.txt",
        [apart::Error::TypeCheck(apart::TypeCheckError::TypeMismatch {
            expected, got
        })] if expected == "Option[Result[bool, i64]]" && got == "Option[Result[unit, i64]]"
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
        unnamed_mod,
        "unnamed_mod.txt",
        [apart::Error::Parse(apart::ParseError::ModWithoutName)]
    );
}
