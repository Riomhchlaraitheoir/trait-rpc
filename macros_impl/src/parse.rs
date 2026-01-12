use crate::{Method, Rpc};
use syn::{
    Attribute, Expr, FnArg, GenericArgument, ItemTrait, Meta, MetaNameValue, PathArguments,
    PathSegment, ReturnType, TraitItem, TraitItemFn, Type, TypeParamBound, TypePath, parse_quote,
};

/// This contains any args in the attribute macro invocation that may affect parsing
// There are no such args for now, but we will keep this just in case tha changes
pub struct Parser;

#[allow(clippy::unused_self)]
impl Parser {
    // TODO: use proc-macro-rules to simplify parsing
    pub fn rpc(&self, input: ItemTrait) -> syn::Result<Rpc> {
        let mut methods = vec![];
        for item in input.items {
            if let TraitItem::Fn(item) = item {
                methods.push(self.method(item)?);
            }
        }
        if !input.supertraits.is_empty() {
            return Err(syn::Error::new_spanned(
                input.supertraits,
                "supertraits are not supported",
            ));
        }
        let docs = input.attrs.iter().filter_map(docs).collect();
        Ok(Rpc {
            docs,
            vis: input.vis,
            generics: input.generics,
            name: input.ident,
            methods,
        })
    }

    fn method(&self, item: TraitItemFn) -> syn::Result<Method> {
        if let Some(default) = item.default {
            return Err(syn::Error::new_spanned(
                default,
                "default fn is not supported",
            ));
        }
        if let Some(con) = item.sig.constness {
            return Err(syn::Error::new_spanned(con, "const fn is not supported"));
        }
        if let Some(unsafety) = item.sig.unsafety {
            return Err(syn::Error::new_spanned(
                unsafety,
                "unsafe fn is not supported",
            ));
        }
        let name = item.sig.ident.clone();
        let mut args = Vec::with_capacity(item.sig.inputs.len() - 1);
        let mut has_self = false;
        for arg in &item.sig.inputs {
            match arg {
                FnArg::Receiver(s) => {
                    if has_self {
                        return Err(syn::Error::new_spanned(s, "cannot have multiple receivers"));
                    }
                    if s.reference.is_none() {
                        return Err(syn::Error::new_spanned(s, "cannot take owned self value"));
                    }
                    if s.mutability.is_some() {
                        return Err(syn::Error::new_spanned(
                            s,
                            "cannot take a mutable self reference",
                        ));
                    }
                    if let Type::Reference(ty) = &*s.ty
                        && ty.mutability.is_none()
                        && let Type::Path(ty) = &*ty.elem
                        && ty.path.segments.len() == 1
                        && ty.path.segments[0].ident == "Self"
                    {
                    } else {
                        return Err(syn::Error::new_spanned(
                            s,
                            "cannot use a smart pointer for self type, must use &Self",
                        ));
                    }
                    has_self = true;
                }
                FnArg::Typed(arg) => args.push(arg.clone()),
            }
        }
        if !has_self {
            return Err(syn::Error::new_spanned(item, "missing self"));
        }
        let ret = self.return_type(item.sig.output)?;
        let docs = item.attrs.iter().filter_map(docs).collect();
        Ok(Method {
            docs,
            name,
            args,
            ret,
        })
    }

    fn return_type(&self, output: ReturnType) -> syn::Result<super::ReturnType> {
        match output {
            ReturnType::Default => Ok(super::ReturnType::Simple(parse_quote! {()})),
            ReturnType::Type(_, ty) => {
                if let Type::ImplTrait(ty) = &*ty {
                    if let Some(first) = ty.bounds.first() {
                        if ty.bounds.len() > 1 {
                            return Err(syn::Error::new_spanned(
                                &ty.bounds,
                                "cannot specify multiple bounds here",
                            ));
                        }
                        if let TypeParamBound::Trait(bound) = first {
                            if bound.lifetimes.is_some() {
                                return Err(syn::Error::new_spanned(
                                    &bound.lifetimes,
                                    "lifetimes not supported here",
                                ));
                            }
                            Ok(super::ReturnType::Nested {
                                service: bound.path.clone(),
                            })
                        } else {
                            Err(syn::Error::new_spanned(ty, "unsupported bound"))
                        }
                    } else {
                        Err(syn::Error::new_spanned(ty, "no bounds found"))
                    }
                } else {
                    if let Type::Path(TypePath { qself: None, path }) = &*ty
                        && path.segments.len() == 1
                    {
                        let PathSegment { ident, arguments } = &path.segments[0];
                        if ident == "Stream"
                            && let PathArguments::AngleBracketed(args) = arguments
                            && args.args.len() == 1
                            && let GenericArgument::Type(ty) = &args.args[0]
                        {
                            return Ok(super::ReturnType::Streaming(ty.clone()));
                        }
                    }
                    Ok(super::ReturnType::Simple(*ty))
                }
            }
        }
    }
}

fn docs(attr: &Attribute) -> Option<Expr> {
    if let Meta::NameValue(MetaNameValue { path, value, .. }) = &attr.meta {
        if path.is_ident("doc") {
            Some(value.clone())
        } else {
            None
        }
    } else {
        None
    }
}

#[cfg(test)]
mod test {
    use crate::parse::Parser;
    use syn::parse_quote;
    use syn::punctuated::Punctuated;
    use syn::token::Paren;
    use syn::{ReturnType, Type, TypeTuple};

    macro_rules! return_type_tests {
        ($($name:ident: $output:expr => {$($input:tt)*}),*) => {
            $(
            #[test]
            fn $name() {
                test_return_type(parse_quote!($($input)*), $output);
            }
            )*
        };
    }

    return_type_tests![
        unit: crate::ReturnType::Simple(Type::Tuple(TypeTuple { paren_token: Paren::default(),elems: Punctuated::default(),})) => {},
        simple: crate::ReturnType::Simple(Type::Path(parse_quote!(String))) => {-> String},
        service: crate::ReturnType::Nested {  service: parse_quote!(SubService) } => { -> impl SubService }
    ];

    #[allow(clippy::needless_pass_by_value)]
    fn test_return_type(input: ReturnType, expected: crate::ReturnType) {
        let parser = Parser;
        let output = parser.return_type(input).expect("failed to parse input");
        assert_eq!(output, expected);
    }
}
