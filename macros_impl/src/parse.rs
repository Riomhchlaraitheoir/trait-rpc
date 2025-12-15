use crate::{Link, Method};
use syn::{parse_quote, FnArg, ItemTrait, ReturnType, TraitItem, Type};

impl TryFrom<ItemTrait> for Link {
    type Error = syn::Error;
    fn try_from(input: ItemTrait) -> Result<Self, Self::Error> {
        let methods = input
            .items
            .into_iter()
            .map(TryInto::try_into)
            .collect::<syn::Result<_>>()?;
        Ok(Self {
            vis: input.vis,
            generics: input.generics,
            name: input.ident,
            colon_token: input.colon_token,
            supertraits: input.supertraits,
            methods,
        })
    }
}

impl TryFrom<TraitItem> for Method {
    type Error = syn::Error;
    fn try_from(item: TraitItem) -> syn::Result<Self> {
        let TraitItem::Fn(item) = item else {
            return Err(syn::Error::new_spanned(
                item,
                "Only fn items are permitted within linked traits",
            ));
        };
        if let Some(default) = item.default {
            return Err(syn::Error::new_spanned(default, "default fn is not supported"));
        }
        if let Some(con) = item.sig.constness {
            return Err(syn::Error::new_spanned(con, "const fn is not supported"));
        }
        if item.sig.asyncness.is_none() {
            return Err(syn::Error::new_spanned(item.sig, "fn items must be async"));
        }
        if let Some(unsafety) = item.sig.unsafety {
            return Err(syn::Error::new_spanned(unsafety, "unsafe fn is not supported"));
        }
        let name = item.sig.ident.clone();
        let mut args = Vec::with_capacity(item.sig.inputs.len()-1);
        let mut has_self = false;
        for arg in &item.sig.inputs {
            match arg {
                FnArg::Receiver(s) => {
                    if has_self {
                        return Err(syn::Error::new_spanned(s, "cannot have multiple receivers"))
                    }
                    if s.reference.is_none() {
                        return Err(syn::Error::new_spanned(s, "cannot take owned self value"))
                    }
                    if s.mutability.is_some() {
                        return Err(syn::Error::new_spanned(s, "cannot take a mutable self reference"))
                    }
                    if let Type::Reference(ty) = &*s.ty &&
                        ty.mutability.is_none() &&
                        let Type::Path(ty) = &*ty.elem &&
                        ty.path.segments.len() == 1 &&
                        ty.path.segments[0].ident == "Self"
                    {

                    } else {
                        return Err(syn::Error::new_spanned(s, "cannot use a smart pointer for self type, must use &Self"))
                    }
                    has_self = true;
                }
                FnArg::Typed(arg) => args.push(arg.clone())
            }
        }
        if !has_self {
            return Err(syn::Error::new_spanned(item, "missing self"))
        }
        let ret = match item.sig.output {
            ReturnType::Default => {
                parse_quote! {()}
            }
            ReturnType::Type(_, ty) => {
                *ty
            }
        };
        let generics = item.sig.generics;
        Ok(Self { name, generics, args, ret })
    }
}
