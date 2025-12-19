use crate::output::Names;
use crate::{Link, Method};
#[cfg(feature = "client")]
use proc_macro2::{Ident, Span};
use std::iter::once;
#[cfg(feature = "client")]
use syn::TypePath;
#[cfg(feature = "client")]
use syn::{AngleBracketedGenericArguments, GenericArgument, Path, PathArguments, PathSegment};
use syn::{FnArg, ItemTrait, ReturnType, Signature, TraitItem, TraitItemFn, Type, parse_quote};
#[cfg(not(feature = "client"))]
use syn::{TraitBound, TraitBoundModifier, TypeImplTrait, TypeParamBound};

impl Link {
    pub fn trait_decl(&self, names: &Names) -> ItemTrait {
        let items =
            self.methods
                .iter()
                .map(|method| method.to_trait_item(names))
                .map(TraitItem::Fn);
        #[cfg(feature = "client")]
        let items = once(TraitItem::Type(parse_quote!(
                type Error: ::core::error::Error;
            )))
            .chain(items);
        let items = items.collect();
        ItemTrait {
            attrs: vec![],
            vis: self.vis.clone(),
            unsafety: None,
            auto_token: None,
            restriction: None,
            trait_token: Default::default(),
            ident: self.name.clone(),
            generics: self.generics.clone(),
            colon_token: self.colon_token,
            supertraits: self.supertraits.clone(),
            brace_token: Default::default(),
            items,
        }
    }
}

impl Method {
    fn to_trait_item(&self, names: &Names) -> TraitItemFn {
        let output = self.return_type(names);
        TraitItemFn {
            attrs: vec![],
            sig: Signature {
                constness: None,
                asyncness: if cfg!(feature = "client") {
                    Some(Default::default())
                } else {
                    None
                },
                unsafety: None,
                abi: None,
                fn_token: Default::default(),
                ident: self.name.clone(),
                generics: self.generics.clone(),
                paren_token: Default::default(),
                inputs: std::iter::once(FnArg::Receiver(parse_quote!(&self)))
                    .chain(self.args.iter().cloned().map(FnArg::Typed))
                    .collect(),
                variadic: None,
                output,
            },
            default: None,
            semi_token: Some(Default::default()),
        }
    }

    #[cfg(feature = "client")]
    fn return_type(&self, names: &Names) -> ReturnType {
        let output = Path {
            leading_colon: None,
            segments: [PathSegment {
                ident: Ident::new("Result", Span::call_site()),
                arguments: PathArguments::AngleBracketed(AngleBracketedGenericArguments {
                    colon2_token: None,
                    lt_token: Default::default(),
                    args: [
                        GenericArgument::Type(self.ret.clone()),
                        GenericArgument::Type(names.link_error(parse_quote!(Self::Error))),
                    ]
                    .into_iter()
                    .collect(),
                    gt_token: Default::default(),
                }),
            }]
            .into_iter()
            .collect(),
        };
        ReturnType::Type(
            Default::default(),
            Box::new(Type::Path(TypePath {
                qself: None,
                path: output,
            })),
        )
    }

    #[cfg(not(feature = "client"))]
    fn return_type(&self, names: &Names) -> ReturnType {
        let output = names.future(self.ret.clone());
        let output = TypeImplTrait {
            impl_token: Default::default(),
            bounds: [
                TypeParamBound::Trait(TraitBound {
                    paren_token: None,
                    modifier: TraitBoundModifier::None,
                    lifetimes: None,
                    path: output,
                }),
                TypeParamBound::Trait(parse_quote!(Send)),
            ]
            .into_iter()
            .collect(),
        };
        ReturnType::Type(Default::default(), Box::new(Type::ImplTrait(output)))
    }
}
