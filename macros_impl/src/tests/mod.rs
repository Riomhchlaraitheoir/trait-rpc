use crate::{rpc, Args};
use proc_macro2::TokenStream;
use quote::{format_ident, ToTokens};
use syn::parse::{Parse, Parser};
use syn::{File, Item, Meta, MetaList};

macro_rules! tests {
    ($($name:ident),*) => {
        $(
        #[test]
        fn $name() {
            let input = include_str!(concat!("inputs/", stringify!($name), ".rs"));
            let expected = include_str!(concat!("outputs/", stringify!($name), ".rs"));
            test_case(input, expected)
        }
        )*
    };
}

fn test_case(input: &'static str, expected: &'static str) {
    let input: TokenStream = input.parse().expect("Failed to parse input");
    let input = File::parse.parse2(input).expect("Failed to parse input");

    let actual = {
        let actual: TokenStream = input.items.into_iter().flat_map(|item| {
            if let Item::Trait(mut item) = item {
                let Some(attr) = item.attrs.first() else {
                    return item.into_token_stream();
                };
                if !attr.path().is_ident(&format_ident!("rpc")) {
                    return item.into_token_stream();
                }
                let args = match &attr.meta {
                    Meta::Path(_) => TokenStream::new(),
                    Meta::List(MetaList { tokens, .. }) => tokens.clone(),
                    Meta::NameValue(_) => {
                        panic!("NameValue attribute type not supported")
                    }
                };
                item.attrs = item.attrs.into_iter().skip(1).collect();
                let args: Args = syn::parse2(args).expect("Failed to parse args");
                match rpc(args, item) {
                    Ok(tokens) => tokens.into_token_stream(),
                    Err(err) => err.into_compile_error(),
                }
            } else {
                item.into_token_stream()
            }
        }).collect();
        let actual_str = actual.to_string();
        let actual = File::parse.parse2(actual).unwrap_or_else(|error| {
            panic!("Failed to parse input: {error}\n{actual_str}");
        });
        prettyplease::unparse(&actual)
    };

    let expected = {
        let expected: TokenStream = expected.parse().expect("Failed to parse input");
        let expected = File::parse.parse2(expected).expect("Failed to parse expected output");
        prettyplease::unparse(&expected)
    };
    difference::assert_diff!(&actual, &expected, "\n", 0);
}

tests!(simple, resource, nested);
