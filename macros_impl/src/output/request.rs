use convert_case::ccase;
use proc_macro2::Ident;
use quote::{format_ident};
use syn::{parse_quote, Field, FieldMutability, Fields, FieldsUnnamed, ItemEnum, Variant, Visibility};
use crate::{Link, Method};
use crate::output::Names;

impl Link {
    pub fn request_name(&self) -> Ident {
        format_ident!("{}Request", &self.name)
    }

    pub fn request_enum(&self, names: &Names) -> ItemEnum {
        let serde = names.serde();
        let name = self.request_name();
        let variants = self.methods.iter().map(Method::request_variant);
        parse_quote!(
            #[derive(Debug, #serde::Serialize, #serde::Deserialize)]
            enum #name {
                #(#variants),*
            }
        )
    }
}

impl Method {
    fn request_variant(&self) -> Variant {
        let name = Ident::new(&ccase!(pascal, self.name.to_string()), self.name.span());
        Variant {
            attrs: vec![],
            ident: name,
            fields: Fields::Unnamed(FieldsUnnamed {
                paren_token: Default::default(),
                unnamed: self.args.iter().map(|pat| Field {
                    attrs: vec![],
                    vis: Visibility::Inherited,
                    mutability: FieldMutability::None,
                    ident: None,
                    colon_token: None,
                    ty: *pat.ty.clone(),
                }).collect(),
            }),
            discriminant: None,
        }
    }
}