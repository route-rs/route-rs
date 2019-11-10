extern crate proc_macro2;
extern crate quote;

use quote::ToTokens;
use std::iter::FromIterator;

/// Indents every non-blank line in the input text by the given string.
pub fn indent<S, T>(indentation: S, text: T) -> String
where
    S: Into<String>,
    T: Into<String>,
{
    let indent_string = indentation.into();
    text.into()
        .lines()
        .map(|l| {
            if !l.is_empty() {
                format!("{}{}", indent_string, String::from(l))
            } else {
                String::from(l)
            }
        })
        .collect::<Vec<String>>()
        .join("\n")
}

#[cfg(test)]
mod indent {
    use super::*;

    #[test]
    fn empty_string() {
        let input = String::from("");
        let output = indent("    ", input);

        assert_eq!(output, "");
    }

    #[test]
    fn oneline_string() {
        let input = String::from("foo asdf bar");
        let output = indent("    ", input);

        assert_eq!(output, "    foo asdf bar");
    }

    #[test]
    fn multiline_string() {
        let input = String::from("foo\nasdf\nbar");
        let output = indent("    ", input);

        assert_eq!(output, "    foo\n    asdf\n    bar");
    }

    #[test]
    fn multiline_string_with_blankline() {
        let input = String::from("foo\n\nasdf\nbar");
        let output = indent("    ", input);

        assert_eq!(output, "    foo\n\n    asdf\n    bar");
    }
}

/// Puts the provided text inside comments
pub fn comment<S: Into<String>>(text: S) -> String {
    text.into()
        .lines()
        .map(|l| format!("// {}", l))
        .collect::<Vec<String>>()
        .join("\n")
}

#[cfg(test)]
mod comment {
    use super::*;

    #[test]
    fn single_line() {
        assert_eq!(comment("Hello world"), "// Hello world");
    }

    #[test]
    fn multi_line() {
        assert_eq!(comment("Hello\nWorld"), "// Hello\n// World")
    }
}

pub fn ident(name: &str) -> syn::Ident {
    syn::Ident::new(name, proc_macro2::Span::call_site())
}

pub fn use_path(name: &str, cont: syn::UseTree) -> syn::UsePath {
    syn::UsePath {
        ident: ident(name),
        colon2_token: syn::token::Colon2 {
            spans: [
                proc_macro2::Span::call_site(),
                proc_macro2::Span::call_site(),
            ],
        },
        tree: Box::new(cont),
    }
}

pub fn use_glob() -> syn::UseGlob {
    syn::UseGlob {
        star_token: syn::token::Star {
            spans: [proc_macro2::Span::call_site()],
        },
    }
}

/// Generates use statements for the provided package imports
pub fn import(imports: &[syn::UseTree]) -> String {
    imports
        .iter()
        .map(|i| {
            syn::Item::Use(syn::ItemUse {
                attrs: vec![],
                vis: syn::Visibility::Inherited,
                use_token: syn::token::Use {
                    span: proc_macro2::Span::call_site(),
                },
                leading_colon: None,
                tree: i.to_owned(),
                semi_token: syn::token::Semi {
                    spans: [proc_macro2::Span::call_site()],
                },
            })
        })
        .map(|i| i.to_token_stream().to_string())
        .collect::<Vec<String>>()
        .join("\n")
}

#[cfg(test)]
mod import {
    use super::*;
    use std::iter::FromIterator;

    fn use_group(subtrees: Vec<syn::UseTree>) -> syn::UseGroup {
        syn::UseGroup {
            brace_token: syn::token::Brace {
                span: proc_macro2::Span::call_site(),
            },
            items: syn::punctuated::Punctuated::from_iter(subtrees.into_iter()),
        }
    }

    #[test]
    fn single_identifier() {
        let tree = syn::UseTree::Name(syn::UseName {
            ident: ident("foobarbaz"),
        });
        assert_eq!(import(&[tree]), "use foobarbaz ;");
    }

    #[test]
    fn multiple_identifier() {
        let trees = &[
            syn::UseTree::Name(syn::UseName {
                ident: ident("foobarbaz"),
            }),
            syn::UseTree::Name(syn::UseName {
                ident: ident("fooasdfbar"),
            }),
            syn::UseTree::Name(syn::UseName {
                ident: ident("barasdffoo"),
            }),
        ];
        assert_eq!(
            import(trees),
            "use foobarbaz ;\nuse fooasdfbar ;\nuse barasdffoo ;"
        );
    }

    #[test]
    fn path_with_glob() {
        let tree = syn::UseTree::Path(use_path("fooasdfbar", syn::UseTree::Glob(use_glob())));
        assert_eq!(import(&[tree]), "use fooasdfbar :: * ;");
    }

    #[test]
    fn path_with_group() {
        let tree = syn::UseTree::Path(use_path(
            "fooasdfbar",
            syn::UseTree::Group(use_group(vec![
                syn::UseTree::Name(syn::UseName {
                    ident: ident("hello"),
                }),
                syn::UseTree::Name(syn::UseName {
                    ident: ident("goodbye"),
                }),
            ])),
        ));
        assert_eq!(import(&[tree]), "use fooasdfbar :: { hello , goodbye } ;");
    }
}

/// Generates a function definition
pub fn function<S, T, U, V, W>(name: S, args: Vec<(T, U)>, return_type: V, body: W) -> String
where
    S: Into<String>,
    T: Into<String>,
    U: Into<String>,
    V: Into<String>,
    W: Into<String>,
{
    let args_formatted = args
        .into_iter()
        .map(|(a, t)| format!("{}: {}", a.into(), t.into()))
        .collect::<Vec<String>>()
        .join(", ");
    let name_and_args = format!("fn {}({})", name.into(), args_formatted);
    let return_type_string = return_type.into();
    let body_block = format!("{{\n{}\n}}", indent("    ", body));
    if return_type_string == "" {
        [name_and_args, body_block].join(" ")
    } else {
        let arrow_return = format!("-> {}", return_type_string);
        [name_and_args, arrow_return, body_block].join(" ")
    }
}

#[cfg(test)]
mod function {
    use super::*;

    #[test]
    fn no_args() {
        let empty_vec: Vec<(&str, &str)> = vec![];
        assert_eq!(
            function("fooasdfbar", empty_vec, "()", "return;"),
            "fn fooasdfbar() -> () {\n    return;\n}"
        );
    }

    #[test]
    fn single_arg() {
        assert_eq!(
            function("fooasdfbar", vec![("foo", "asdf::Bar")], "()", "return;"),
            "fn fooasdfbar(foo: asdf::Bar) -> () {\n    return;\n}"
        );
    }

    #[test]
    fn multiple_args() {
        assert_eq!(
            function(
                "fooasdfbar",
                vec![("foo", "asdf::Bar"), ("bar", "asdf::Foo")],
                "()",
                "return;"
            ),
            "fn fooasdfbar(foo: asdf::Bar, bar: asdf::Foo) -> () {\n    return;\n}"
        );
    }

    #[test]
    fn blank_return_type() {
        let empty_vec: Vec<(&str, &str)> = vec![];
        assert_eq!(
            function("fooasdfbar", empty_vec, "", "return;"),
            "fn fooasdfbar() {\n    return;\n}"
        );
    }
}

pub fn impl_struct<S, T, U>(trait_name: S, struct_name: T, body: U) -> String
where
    S: Into<String>,
    T: Into<String>,
    U: Into<String>,
{
    let trait_name_string = trait_name.into();
    if trait_name_string == "" {
        format!(
            "impl {} {{\n{}\n}}",
            struct_name.into(),
            indent("    ", body)
        )
    } else {
        format!(
            "impl {} for {} {{\n{}\n}}",
            trait_name_string,
            struct_name.into(),
            indent("    ", body)
        )
    }
}

#[cfg(test)]
mod impl_struct {
    use super::*;

    #[test]
    fn has_trait() {
        assert_eq!(
            impl_struct("foo::asdf::Bar", "Quux", "type Baz = usize;"),
            "impl foo::asdf::Bar for Quux {\n    type Baz = usize;\n}"
        );
    }

    #[test]
    fn no_trait() {
        assert_eq!(
            impl_struct("", "Quux", "type Baz = usize;"),
            "impl Quux {\n    type Baz = usize;\n}"
        );
    }
}

pub fn simple_path(segments: Vec<syn::Ident>, with_leading_colon: bool) -> syn::Path {
    syn::Path {
        leading_colon: if with_leading_colon {
            Some(syn::token::Colon2 {
                spans: [
                    proc_macro2::Span::call_site(),
                    proc_macro2::Span::call_site(),
                ],
            })
        } else {
            None
        },
        segments: syn::punctuated::Punctuated::from_iter(segments.into_iter().map(|s| {
            syn::PathSegment {
                ident: s,
                arguments: syn::PathArguments::None,
            }
        })),
    }
}

/// Generates type declaration statements
pub fn typedef(types: Vec<(syn::Ident, syn::Type)>) -> String {
    types
        .into_iter()
        .map(|(new_type, existing_type)| syn::ItemType {
            attrs: vec![],
            vis: syn::Visibility::Inherited,
            type_token: syn::token::Type {
                span: proc_macro2::Span::call_site(),
            },
            ident: new_type,
            generics: syn::Generics {
                lt_token: None,
                params: syn::punctuated::Punctuated::new(),
                gt_token: None,
                where_clause: None,
            },
            eq_token: syn::token::Eq {
                spans: [proc_macro2::Span::call_site()],
            },
            ty: Box::new(existing_type),
            semi_token: syn::token::Semi {
                spans: [proc_macro2::Span::call_site()],
            },
        })
        .map(|t| t.to_token_stream().to_string())
        .collect::<Vec<String>>()
        .join("\n")
}

#[cfg(test)]
mod typedef {
    use super::*;

    #[test]
    fn single_import() {
        assert_eq!(
            typedef(vec![(
                ident("FooAsdfBar"),
                syn::Type::Path(syn::TypePath {
                    qself: None,
                    path: simple_path(vec![ident("usize")], false)
                })
            )]),
            "type FooAsdfBar = usize ;"
        );
    }

    #[test]
    fn multi_import() {
        assert_eq!(
            typedef(vec![
                (
                    ident("FooAsdfBar"),
                    syn::Type::Path(syn::TypePath {
                        qself: None,
                        path: simple_path(vec![ident("usize")], false)
                    })
                ),
                (
                    ident("BarAsdfFoo"),
                    syn::Type::Path(syn::TypePath {
                        qself: None,
                        path: simple_path(vec![ident("isize")], false)
                    })
                )
            ]),
            "type FooAsdfBar = usize ;\ntype BarAsdfFoo = isize ;"
        );
    }
}

pub fn angle_bracketed_types(types: Vec<syn::Type>) -> syn::AngleBracketedGenericArguments {
    syn::AngleBracketedGenericArguments {
        colon2_token: None,
        lt_token: syn::token::Lt {
            spans: [proc_macro2::Span::call_site()],
        },
        args: syn::punctuated::Punctuated::from_iter(
            types.into_iter().map(syn::GenericArgument::Type),
        ),
        gt_token: syn::token::Gt {
            spans: [proc_macro2::Span::call_site()],
        },
    }
}

pub fn let_simple(
    identifier: syn::Ident,
    type_annotation: Option<syn::Type>,
    expression: syn::Expr,
    mutable: bool,
) -> syn::Local {
    let id = syn::Pat::Ident(syn::PatIdent {
        attrs: vec![],
        by_ref: None,
        mutability: if mutable {
            Some(syn::token::Mut {
                span: proc_macro2::Span::call_site(),
            })
        } else {
            None
        },
        ident: identifier,
        subpat: None,
    });
    syn::Local {
        attrs: vec![],
        let_token: syn::token::Let {
            span: proc_macro2::Span::call_site(),
        },
        pat: match type_annotation {
            None => id,
            Some(ty) => syn::Pat::Type(syn::PatType {
                attrs: vec![],
                pat: Box::new(id),
                colon_token: syn::token::Colon {
                    spans: [proc_macro2::Span::call_site()],
                },
                ty: Box::new(ty),
            }),
        },
        init: Some((
            syn::token::Eq {
                spans: [proc_macro2::Span::call_site()],
            },
            Box::new(expression),
        )),
        semi_token: syn::token::Semi {
            spans: [proc_macro2::Span::call_site()],
        },
    }
}

pub fn let_new<S, T, U>(identifier: S, struct_name: T, args: Vec<U>) -> String
where
    S: Into<String>,
    T: Into<String>,
    U: Into<String>,
{
    let args_string = args
        .into_iter()
        .map(|a| a.into())
        .collect::<Vec<String>>()
        .join(", ");
    format!(
        "let {} = {}::new({});",
        identifier.into(),
        struct_name.into(),
        args_string
    )
}

#[cfg(test)]
mod let_new {
    use super::*;

    #[test]
    fn no_args() {
        let empty_args: Vec<&str> = vec![];
        assert_eq!(let_new("foo", "Asdf", empty_args), "let foo = Asdf::new();")
    }

    #[test]
    fn single_arg() {
        assert_eq!(
            let_new("foo", "Asdf", vec!["12345"]),
            "let foo = Asdf::new(12345);"
        )
    }

    #[test]
    fn multiple_args() {
        assert_eq!(
            let_new("foo", "Asdf", vec!["12345", "true"]),
            "let foo = Asdf::new(12345, true);"
        )
    }
}

pub fn match_expr<S, T, U>(symbol: S, branches: Vec<(T, U)>) -> String
where
    S: Into<String>,
    T: Into<String>,
    U: Into<String>,
{
    let branch_expr = branches
        .into_iter()
        .map(|(pattern, result)| format!("{} => {{ {} }},", pattern.into(), result.into()))
        .collect::<Vec<String>>();
    let branch_expr_str = if !branch_expr.is_empty() {
        [
            String::from("\n"),
            indent("    ", branch_expr.join("\n")),
            String::from("\n"),
        ]
        .join("")
    } else {
        String::from("")
    };
    format!("match {} {{{}}}", symbol.into(), branch_expr_str)
}

#[cfg(test)]
mod match_expr {
    use super::*;

    #[test]
    fn empty_branches() {
        let empty_branches: Vec<(&str, &str)> = vec![];
        assert_eq!(match_expr("foo", empty_branches), "match foo {}");
    }

    #[test]
    fn one_branch() {
        let branches = vec![("_", "true")];
        assert_eq!(
            match_expr("foo", branches),
            "match foo {\n    _ => { true },\n}"
        );
    }

    #[test]
    fn two_branches() {
        let branches = vec![("true", "\"red\""), ("false", "\"blue\"")];
        assert_eq!(
            match_expr("foo", branches),
            "match foo {\n    true => { \"red\" },\n    false => { \"blue\" },\n}"
        );
    }
}

pub fn box_expr<S>(expr: S) -> String
where
    S: Into<String>,
{
    format!("Box::new({})", expr.into())
}

#[cfg(test)]
mod box_expr {
    use super::*;

    #[test]
    fn thing() {
        assert_eq!(box_expr("thing"), "Box::new(thing)");
    }
}
