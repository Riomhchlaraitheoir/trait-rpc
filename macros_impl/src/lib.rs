use proc_macro2::{Ident, TokenStream};
use quote::ToTokens;
use syn::{Generics, ItemTrait, PatType, Token, Type, TypeParamBound, Visibility};
use syn::punctuated::Punctuated;

#[cfg(test)]
mod tests;
mod parse;

mod output;

pub fn link(_args: TokenStream, input: ItemTrait) -> syn::Result<impl ToTokens> {
    Link::try_from(input)
}

struct Link {
    vis: Visibility,
    generics: Generics,
    name: Ident,
    colon_token: Option<Token![:]>,
    supertraits: Punctuated<TypeParamBound, Token![+]>,
    methods: Vec<Method>
}

struct Method {
    name: Ident,
    generics: Generics,
    args: Vec<PatType>,
    ret: Type
}
