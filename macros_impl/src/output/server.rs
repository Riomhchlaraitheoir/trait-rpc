use crate::{Link, Method};
use convert_case::ccase;
use proc_macro2::Ident;
use quote::format_ident;
use syn::{parse_quote, Arm, Field, FieldMutability, Fields, FieldsUnnamed, Item, ItemImpl, ItemStruct, Pat, Visibility};
use crate::output::Names;

impl Link {
    pub fn server_name(&self) -> Ident {
        format_ident!("{}Server", self.name)
    }

    pub fn server(&self, names: &Names) -> Vec<Item> {
        vec![
            Item::Struct(self.server_struct()),
            Item::Impl(self.server_new()),
            Item::Impl(self.server_impl(names))
        ]
    }

    fn server_struct(&self) -> ItemStruct {
        let mut generics = self.generics.clone();
        let rpc = &self.name;
        generics.params.push(parse_quote!(T: #rpc));
        ItemStruct {
            attrs: vec![],
            vis: self.vis.clone(),
            struct_token: Default::default(),
            ident: self.server_name(),
            generics,
            fields: Fields::Unnamed(FieldsUnnamed {
                paren_token: Default::default(),
                unnamed: [
                    Field {
                        attrs: vec![],
                        vis: Visibility::Inherited,
                        mutability: FieldMutability::None,
                        ident: None,
                        colon_token: None,
                        ty: parse_quote!(T),
                    }
                ].into_iter().collect(),
            }),
            semi_token: None,
        }
    }

    fn server_new(&self) -> ItemImpl {
        let rpc = &self.name;
        let server = self.server_name();
        let vis = &self.vis;
        parse_quote! {
            impl<T: #rpc> #server<T> {
                #vis fn new(server: T) -> Self {
                    Self(server)
                }
            }
        }
    }

    fn server_impl(&self, names: &Names) -> ItemImpl {
        let rpc = &self.name;
        let server = self.server_name();
        let request = self.request_name();
        let response = self.response_name();
        let methods = self.methods.iter().map(|method| method.rpc_case(&request, &response));
        let rpc_trait = names.rpc_trait();
        parse_quote! {
            impl<T: #rpc + Sync> #rpc_trait for #server<T> {
                type Request = #request;
                type Response = #response;

                async fn process(&self, request: Self::Request) -> Self::Response {
                    #[allow(clippy::unit_arg)]
                    match request {
                        #(#methods)*
                    }
                }
            }
        }
    }
}

impl Method {
    fn rpc_case(&self, request: &Ident, response: &Ident) -> Arm {
        let name = &self.name;
        let variant = Ident::new(&ccase!(pascal, self.name.to_string()), self.name.span());
        let pats: Vec<Pat> = self.args.iter().map(|arg| (*arg.pat).clone()).collect();
        parse_quote! {
            #request::#variant(#(#pats),*) => {
                #response::#variant(self.0.#name(#(#pats),*).await)
            }
        }
    }
}