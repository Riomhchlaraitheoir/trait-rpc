use std::path::Path;
use quote::{format_ident, quote, ToTokens};
use proc_macro2::TokenStream;
use crate::link;
use syn::parse::{Parse, ParseStream, Parser};
use syn::{parse_quote, File, Item, ItemTrait, Meta, MetaList, MetaNameValue};

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
    let mut input = File::parse.parse2(input).expect("Failed to parse input");

    let actual = {
        let actual = input.items.into_iter().flat_map(|item| {
            if let Item::Trait(mut item) = item {
                let Some(attr) = item.attrs.first() else {
                    return item.into_token_stream();
                };
                if !attr.path().is_ident(&format_ident!("trait_link")) {
                    return item.into_token_stream();
                }
                let args = match &attr.meta {
                    Meta::Path(_) => TokenStream::new(),
                    Meta::List(MetaList { tokens, .. }) => tokens.clone(),
                    Meta::NameValue(meta) => {
                        panic!("NameValue attribute type not supported")
                    }
                };
                item.attrs = item.attrs.into_iter().skip(1).collect();
                match link(args, item) {
                    Ok(tokens) => tokens.into_token_stream(),
                    Err(err) => err.into_compile_error(),
                }
            } else {
                item.into_token_stream()
            }
        }).collect();
        let actual = File::parse.parse2(actual).expect("Failed to parse input");
        prettyplease::unparse(&actual)
    };

    let expected = {
        let expected: TokenStream = expected.parse().expect("Failed to parse input");
        let expected = File::parse.parse2(expected).expect("Failed to parse input");
        prettyplease::unparse(&expected)
    };
    difference::assert_diff!(&actual, &expected, "\n", 0);
}

tests!(simple, public);
