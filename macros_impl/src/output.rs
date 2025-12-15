mod trait_decl;
mod request;
mod response;
#[cfg(not(feature = "client"))]
mod server;
#[cfg(feature = "client")]
mod client;

use crate::Link;
use proc_macro2::TokenStream;
use quote::ToTokens;
use syn::{parse_quote, Item, Path, Type};

impl ToTokens for Link {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let names = Names;
        let mut items = Vec::new();
        items.push(Item::Trait(self.trait_decl(&names)));
        items.push(Item::Enum(self.request_enum(&names)));
        items.push(Item::Enum(self.response_enum(&names)));
        #[cfg(feature = "client")]
        items.extend(self.client(&names));
        #[cfg(not(feature = "client"))]
        items.extend(self.server(&names));
        
        tokens.extend(items.into_iter().flat_map(ToTokens::into_token_stream));
    }
    
}

pub struct Names;

impl Names {
    #[cfg(feature = "client")]
    fn transport(&self) -> Type {
        parse_quote!(::trait_link::Transport)
    }

    #[cfg(feature = "client")]
    fn link_error(&self, transport_error: Type) -> Type {
       parse_quote!(::trait_link::LinkError::<#transport_error>)
    }

    fn serde(&self) -> Path {
        parse_quote!(::trait_link::serde)
    }

    #[cfg(not(feature = "client"))]
    fn future(&self, output: Type) -> Path {
        parse_quote!(::core::future::Future<Output = #output>)
    }

    #[cfg(not(feature = "client"))]
    fn rpc_trait(&self) -> Path {
        parse_quote!(::trait_link::Rpc)
    }
}
