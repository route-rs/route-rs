pub(crate) fn ident(name: &str) -> syn::Ident {
    syn::Ident::new(name, fake_span!())
}

pub(crate) fn lit_str(contents: &str) -> syn::Lit {
    syn::Lit::Str(syn::LitStr::new(contents, fake_span!()))
}
