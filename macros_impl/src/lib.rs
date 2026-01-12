use crate::parse::Parser;
use proc_macro2::{Ident, TokenStream};
use quote::ToTokens;
use syn::{Expr, Generics, ItemTrait, PatType, Path, Type, Visibility};

#[cfg(test)]
mod tests;
mod parse;

mod output;

/// The function to invoke the rpc macro
/// 
/// # Errors
/// Can return a [`syn::Error`] if it fails to parse the input or rejects some part of the input
pub fn rpc(_args: TokenStream, input: ItemTrait) -> syn::Result<impl ToTokens> {
    let parser = Parser;
    parser.rpc(input)
}

struct Rpc {
    docs: Vec<Expr>,
    vis: Visibility,
    generics: Generics,
    name: Ident,
    methods: Vec<Method>,
}

struct Method {
    docs: Vec<Expr>,
    name: Ident,
    args: Vec<PatType>,
    ret: ReturnType,
}

#[derive(Debug, PartialEq, Eq)]
enum ReturnType {
    Simple(Type),
    Nested { service: Path },
    Streaming(Type),
}
