use quote::ToTokens;
use proc_macro2::TokenStream;
use crate::link;
use syn::parse::{Parse, Parser};
use syn::{File, ItemTrait};

macro_rules! tests {
    ($($name:ident),*) => {
        $(
        #[test]
        fn $name() {
            let input = include_str!(concat!("inputs/", stringify!($name), ".rs"));
            #[cfg(feature = "client")]
            let expected = include_str!(concat!("outputs_client/", stringify!($name), ".rs"));
            #[cfg(not(feature = "client"))]
            let expected = include_str!(concat!("outputs_server/", stringify!($name), ".rs"));
            test_case(input, expected)
        }
        )*
    };
}

fn test_case(input: &'static str, expected: &'static str) {
    let input: TokenStream = input.parse().expect("Failed to parse input");
    let input = ItemTrait::parse.parse2(input).expect("Failed to parse input");
    let expected: TokenStream = expected.parse().expect("Failed to parse input");
    let expected = File::parse.parse2(expected).expect("Failed to parse input");
    let expected = prettyplease::unparse(&expected);
    let actual: TokenStream = match link(TokenStream::new(), input) {
        Ok(tokens) => tokens.into_token_stream(),
        Err(err) => err.into_compile_error(),
    };
    let actual = File::parse.parse2(actual).expect("Failed to parse input");
    let actual = prettyplease::unparse(&actual);
    difference::assert_diff!(&actual, &expected, "\n", 0);
}

tests!(simple);
