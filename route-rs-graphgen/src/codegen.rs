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

/// Generates use statements for the provided package imports
pub fn import<S: Into<String>>(imports: Vec<S>) -> String {
    imports
        .into_iter()
        .map(|i| format!("use {};", i.into()))
        .collect::<Vec<String>>()
        .join("\n")
}

#[cfg(test)]
mod import {
    use super::*;

    #[test]
    fn single_import() {
        assert_eq!(import(vec!["package::function"]), "use package::function;");
    }

    #[test]
    fn multi_import() {
        assert_eq!(
            import(vec!["package::somefunction", "library::otherfunction"]),
            "use package::somefunction;\nuse library::otherfunction;"
        );
    }

    #[test]
    fn brace_notation() {
        assert_eq!(
            import(vec!["package::{somefunction, otherfunction}"]),
            "use package::{somefunction, otherfunction};"
        );
    }

    #[test]
    fn wildcards() {
        assert_eq!(import(vec!["package::*"]), "use package::*;");
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

/// Generates type declaration statements
pub fn typedef<S, T>(types: Vec<(S, T)>) -> String
where
    S: Into<String>,
    T: Into<String>,
{
    types
        .into_iter()
        .map(|(new_type, existing_type)| {
            format!("type {} = {};", new_type.into(), existing_type.into())
        })
        .collect::<Vec<String>>()
        .join("\n")
}

#[cfg(test)]
mod typedef {
    use super::*;

    #[test]
    fn single_import() {
        assert_eq!(
            typedef(vec![("FooAsdfBar", "usize")]),
            "type FooAsdfBar = usize;"
        );
    }

    #[test]
    fn multi_import() {
        assert_eq!(
            typedef(vec![("FooAsdfBar", "usize"), ("BarAsdfFoo", "isize")]),
            "type FooAsdfBar = usize;\ntype BarAsdfFoo = isize;"
        );
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
