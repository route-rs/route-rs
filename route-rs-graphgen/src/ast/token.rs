macro_rules! fake_span {
    () => {
        proc_macro2::Span::call_site()
    };
}

macro_rules! gen_token {
    ($token:ident + span) => {
        syn::token::$token { span: fake_span!() }
    };

    ($token:ident + spans1) => {
        syn::token::$token {
            spans: [fake_span!()],
        }
    };

    ($token:ident + spans2) => {
        syn::token::$token {
            spans: [fake_span!()],
        }
    };
}
