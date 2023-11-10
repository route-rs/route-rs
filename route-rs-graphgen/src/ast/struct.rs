use crate::ast;
use std::iter::FromIterator;

pub(crate) fn def_struct(visibility: syn::Visibility, name: &str, fields: Vec<syn::Field>) -> syn::Item {
    syn::Item::Struct(syn::ItemStruct {
        attrs: vec![],
        vis: visibility,
        struct_token: gen_token!(Struct + span),
        ident: ast::ident(name),
        generics: Default::default(),
        fields: syn::Fields::Named(syn::FieldsNamed {
            brace_token: gen_token!(Brace + span),
            named: syn::punctuated::Punctuated::from_iter(fields),
        }),
        semi_token: None,
    })
}

pub(crate) fn vis_pub_crate() -> syn::Visibility {
    syn::Visibility::Restricted(syn::VisRestricted {
        pub_token: gen_token!(Pub + span),
        paren_token: gen_token!(Paren + span),
        in_token: None,
        path: Box::new(syn::Path {
            leading_colon: None,
            segments: syn::punctuated::Punctuated::from_iter(vec![
                syn::PathSegment { ident: ast::ident("crate"), arguments: Default::default() }
            ])
        })
    })
}