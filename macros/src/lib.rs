use proc_macro::TokenStream;
use syn::{parse_macro_input, ItemTrait};
use quote::ToTokens;

#[proc_macro_attribute]
pub fn trait_link(args: TokenStream, input: TokenStream) -> TokenStream {
    let link = parse_macro_input!(input as ItemTrait);

    match macros_impl::link(args.into(), link) {
        Ok(tokens) => tokens.into_token_stream(),
        Err(err) => err.to_compile_error(),
    }.into()
}