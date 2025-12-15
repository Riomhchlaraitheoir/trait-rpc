use crate::output::Names;
use crate::{Link, Method};
use convert_case::ccase;
use proc_macro2::Ident;
use quote::format_ident;
use syn::{parse_quote, ImplItemFn, Item, ItemImpl, ItemStruct, Pat};

impl Link {
    pub fn client_name(&self) -> Ident {
        format_ident!("{}Client", self.name)
    }

    pub fn client(&self, names: &Names) -> Vec<Item> {
        vec![
            Item::Struct(self.client_struct(names)),
            Item::Impl(self.client_impl(names)),
            Item::Impl(self.client_trait_impl(names))
        ]
    }

    fn client_struct(&self, names: &Names) -> ItemStruct {
        let name = self.client_name();
        let vis = &self.vis;
        let transport = names.transport();
        parse_quote! {
            #vis struct #name<T: #transport>(T);
        }
    }

    fn client_impl(&self, names: &Names) -> ItemImpl {
        let client = self.client_name();
        let vis = &self.vis;
        let transport = names.transport();
        parse_quote! {
            impl<T: #transport> #client<T> {
                #vis fn new(transport: T) -> Self {
                    Self(transport)
                }
            }
        }
    }

    fn client_trait_impl(&self, names: &Names) -> ItemImpl {
        let rpc = &self.name;
        let client = self.client_name();
        let transport = names.transport();
        let request = self.request_name();
        let response = self.response_name();
        let methods = self.methods.iter().map(|method| method.client_fn(&request, &response, names));
        parse_quote! {
            impl<T: #transport> #rpc for #client<T> {
                type Error = <T as #transport>::Error;
                #(#methods)*
            }
        }
    }
}

impl Method {
    fn client_fn(&self, request: &Ident, response: &Ident, names: &Names) -> ImplItemFn {
        let name = &self.name;
        let variant = Ident::new(&ccase!(pascal, self.name.to_string()), self.name.span());
        let pats: Vec<Pat> = self.args.iter().map(|arg| (*arg.pat).clone()).collect();
        let params = self.args.iter();
        let ret = &self.ret;
        let transport = names.transport();
        let link_error = names.link_error(parse_quote!(T::Error));
        parse_quote! {
            async fn #name(&self, #(#params),*) -> Result<#ret, #link_error> {
                if let #response::#variant(value) = self.0.send(#request::#variant(#(#pats),*)).await? {
                    Ok(value)
                } else {
                    Err(#link_error::WrongResponseType)
                }
            }
        }
    }
}