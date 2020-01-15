extern crate proc_macro2;
extern crate quote;

use quote::ToTokens;
use std::iter::FromIterator;

/// We need Span structs all over the place because syn expects to be used as a parser. We're using
/// it for pure codegen, so we have to pretend that we parsed our code somewhere. Conveniently, we
/// can just say that somewhere is right here, so we will.
fn fake_span() -> proc_macro2::Span {
    proc_macro2::Span::call_site()
}

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
    syn::Ident::new(name, fake_span())
}

pub fn use_path(name: &str, cont: syn::UseTree) -> syn::UsePath {
    syn::UsePath {
        ident: ident(name),
        colon2_token: syn::token::Colon2 {
            spans: [fake_span(), fake_span()],
        },
        tree: Box::new(cont),
    }
}

pub fn use_glob() -> syn::UseGlob {
    syn::UseGlob {
        star_token: syn::token::Star {
            spans: [fake_span()],
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
                use_token: syn::token::Use { span: fake_span() },
                leading_colon: None,
                tree: i.to_owned(),
                semi_token: syn::token::Semi {
                    spans: [fake_span()],
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
            brace_token: syn::token::Brace { span: fake_span() },
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
                spans: [fake_span(), fake_span()],
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
            type_token: syn::token::Type { span: fake_span() },
            ident: new_type,
            generics: syn::Generics {
                lt_token: None,
                params: syn::punctuated::Punctuated::new(),
                gt_token: None,
                where_clause: None,
            },
            eq_token: syn::token::Eq {
                spans: [fake_span()],
            },
            ty: Box::new(existing_type),
            semi_token: syn::token::Semi {
                spans: [fake_span()],
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
            spans: [fake_span()],
        },
        args: syn::punctuated::Punctuated::from_iter(
            types.into_iter().map(syn::GenericArgument::Type),
        ),
        gt_token: syn::token::Gt {
            spans: [fake_span()],
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
            Some(syn::token::Mut { span: fake_span() })
        } else {
            None
        },
        ident: identifier,
        subpat: None,
    });
    syn::Local {
        attrs: vec![],
        let_token: syn::token::Let { span: fake_span() },
        pat: match type_annotation {
            None => id,
            Some(ty) => syn::Pat::Type(syn::PatType {
                attrs: vec![],
                pat: Box::new(id),
                colon_token: syn::token::Colon {
                    spans: [fake_span()],
                },
                ty: Box::new(ty),
            }),
        },
        init: Some((
            syn::token::Eq {
                spans: [fake_span()],
            },
            Box::new(expression),
        )),
        semi_token: syn::token::Semi {
            spans: [fake_span()],
        },
    }
}

pub fn expr_path_ident(id: &str) -> syn::Expr {
    syn::Expr::Path(syn::ExprPath {
        attrs: vec![],
        qself: None,
        path: simple_path(vec![ident(id)], false),
    })
}

pub fn builder(base: syn::Ident, setters: Vec<(syn::Ident, Vec<syn::Expr>)>) -> syn::Expr {
    let mut expr_accum = syn::Expr::Call(syn::ExprCall {
        attrs: vec![],
        func: Box::new(syn::Expr::Path(syn::ExprPath {
            attrs: vec![],
            qself: None,
            path: simple_path(vec![base, ident("new")], false),
        })),
        paren_token: syn::token::Paren { span: fake_span() },
        args: Default::default(),
    });

    for (method, args) in setters {
        expr_accum = syn::Expr::MethodCall(syn::ExprMethodCall {
            attrs: vec![],
            receiver: Box::new(expr_accum),
            dot_token: syn::token::Dot {
                spans: [fake_span()],
            },
            method,
            turbofish: None,
            paren_token: syn::token::Paren { span: fake_span() },
            args: syn::punctuated::Punctuated::from_iter(args.into_iter()),
        })
    }

    expr_accum = syn::Expr::MethodCall(syn::ExprMethodCall {
        attrs: vec![],
        receiver: Box::new(expr_accum),
        dot_token: syn::token::Dot {
            spans: [fake_span()],
        },
        method: ident("build_link"),
        turbofish: None,
        paren_token: syn::token::Paren { span: fake_span() },
        args: syn::punctuated::Punctuated::from_iter(Vec::<syn::Expr>::new().into_iter()),
    });

    expr_accum
}

pub fn build_link(
    index: usize,
    link_type: &str,
    setters: Vec<(syn::Ident, Vec<syn::Expr>)>,
    num_egressors: usize,
) -> Vec<syn::Stmt> {
    let mut stmts = vec![];

    stmts.push(syn::Stmt::Local(syn::Local {
        attrs: vec![],
        let_token: syn::token::Let { span: fake_span() },
        pat: syn::Pat::Tuple(syn::PatTuple {
            attrs: vec![],
            paren_token: syn::token::Paren { span: fake_span() },
            elems: syn::punctuated::Punctuated::from_iter(
                vec![
                    syn::Pat::Ident(syn::PatIdent {
                        attrs: vec![],
                        by_ref: None,
                        mutability: Some(syn::token::Mut { span: fake_span() }),
                        ident: ident(format!("runnables_{}", &index).as_str()),
                        subpat: None,
                    }),
                    syn::Pat::Ident(syn::PatIdent {
                        attrs: vec![],
                        by_ref: None,
                        mutability: Some(syn::token::Mut { span: fake_span() }),
                        ident: if num_egressors > 0 {
                            ident(format!("egressors_{}", &index).as_str())
                        } else {
                            ident(format!("_egressors_{}", &index).as_str())
                        },
                        subpat: None,
                    }),
                ]
                .into_iter(),
            ),
        }),
        init: Some((
            syn::token::Eq {
                spans: [fake_span()],
            },
            Box::new(builder(ident(link_type), setters)),
        )),
        semi_token: syn::token::Semi {
            spans: [fake_span()],
        },
    }));

    stmts.push(syn::Stmt::Semi(
        syn::Expr::MethodCall(syn::ExprMethodCall {
            attrs: vec![],
            receiver: Box::new(syn::Expr::Path(syn::ExprPath {
                attrs: vec![],
                qself: None,
                path: simple_path(vec![ident("all_runnables")], false),
            })),
            dot_token: syn::token::Dot {
                spans: [fake_span()],
            },
            method: ident("append"),
            turbofish: None,
            paren_token: syn::token::Paren { span: fake_span() },
            args: syn::punctuated::Punctuated::from_iter(vec![syn::Expr::Reference(
                syn::ExprReference {
                    attrs: vec![],
                    and_token: syn::token::And {
                        spans: [fake_span()],
                    },
                    raw: Default::default(),
                    mutability: Some(syn::token::Mut { span: fake_span() }),
                    expr: Box::new(syn::Expr::Path(syn::ExprPath {
                        attrs: vec![],
                        qself: None,
                        path: simple_path(
                            vec![ident(format!("runnables_{}", &index).as_str())],
                            false,
                        ),
                    })),
                },
            )]),
        }),
        syn::token::Semi {
            spans: [fake_span()],
        },
    ));

    for n in 0..num_egressors {
        stmts.push(syn::Stmt::Local(let_simple(
            ident(format!("link_{}_egress_{}", &index, n).as_str()),
            None,
            syn::Expr::MethodCall(syn::ExprMethodCall {
                attrs: vec![],
                receiver: Box::new(syn::Expr::Path(syn::ExprPath {
                    attrs: vec![],
                    qself: None,
                    path: simple_path(vec![ident(format!("egressors_{}", &index).as_str())], false),
                })),
                dot_token: syn::token::Dot {
                    spans: [fake_span()],
                },
                method: ident("remove"),
                turbofish: None,
                paren_token: syn::token::Paren { span: fake_span() },
                args: syn::punctuated::Punctuated::from_iter(vec![syn::Expr::Lit(syn::ExprLit {
                    attrs: vec![],
                    lit: syn::Lit::Int(syn::LitInt::new("0", fake_span())),
                })]),
            }),
            false,
        )))
    }

    stmts
}

pub fn vec(exprs: Vec<syn::Expr>) -> syn::Expr {
    syn::Expr::Macro(syn::ExprMacro {
        attrs: vec![],
        mac: syn::Macro {
            path: simple_path(vec![ident("vec")], false),
            bang_token: syn::token::Bang {
                spans: [fake_span()],
            },
            delimiter: syn::MacroDelimiter::Bracket(syn::token::Bracket { span: fake_span() }),
            tokens: syn::punctuated::Punctuated::<syn::Expr, syn::token::Comma>::from_iter(exprs)
                .to_token_stream(),
        },
    })
}
