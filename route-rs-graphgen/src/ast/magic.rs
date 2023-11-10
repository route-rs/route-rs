use crate::ast;
use std::iter::FromIterator;
use syn::export::ToTokens;

const MAGIC_COMMENT_SIGIL: &str = "graphgen_magic_comment";
const MAGIC_COMMENT_REGEX: &str =
    r#"graphgen_magic_comment\s*!\s*\(\s*"(?P<comment>.+?)"\s*\)\s*;"#;

pub(crate) fn magic_comment(comment: &str) -> syn::Item {
    syn::Item::Macro(syn::ItemMacro {
        attrs: vec![],
        ident: None,
        mac: syn::Macro {
            path: syn::Path {
                leading_colon: None,
                segments: syn::punctuated::Punctuated::from_iter(vec![syn::PathSegment {
                    ident: ast::ident(MAGIC_COMMENT_SIGIL),
                    arguments: Default::default(),
                }]),
            },
            bang_token: gen_token!(Bang + spans1),
            delimiter: syn::MacroDelimiter::Paren(gen_token!(Paren + span)),
            tokens: ast::lit_str(comment).to_token_stream(),
        },
        semi_token: Some(gen_token!(Semi + spans1)),
    })
}

pub(crate) fn unmagic_comments(source: &str) -> String {
    let re: regex::Regex = regex::Regex::new(MAGIC_COMMENT_REGEX).unwrap();

    String::from(re.replace_all(source, "// $comment\n"))
}
