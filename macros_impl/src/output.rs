use crate::{ReturnType, Rpc};
use convert_case::ccase;
use proc_macro2::{Ident, Span, TokenStream};
use quote::{ToTokens, format_ident, quote};
use syn::{Field, FieldMutability, Visibility, LitStr, GenericParam};

macro_rules! ident_ccase {
    ($case:ident, $ident:expr) => {
        Ident::new(&ccase!($case, $ident.to_string()), $ident.span())
    };
}

impl ToTokens for Rpc {
    #[allow(
        clippy::too_many_lines,
        reason = "This is a long function, but not too complex and splitting it would likely make it more confusing not less"
    )]
    fn to_token_stream(&self) -> TokenStream {
        let trait_rpc = &self.args.trait_rpc;
        let serde = &self.args.serde;
        let service = &self.name;
        let module = ident_ccase!(snake, service);

        let generics_with_bounds = &self.generics;
        let gen_params_with_bounds: Vec<_> = generics_with_bounds.params.iter().collect();
        let gen_params: Vec<_> = generics_with_bounds.params.iter().map(|p| {
            if let GenericParam::Type(p) = p {
                p.ident.to_token_stream()
            } else {
                p.to_token_stream()
            }
        }).collect();
        let maybe_generics = if gen_params.is_empty() {
            vec![]
        } else {
            vec![gen_params.clone()]
        };
        let generics = quote!(#(<#(#maybe_generics),*>)*);

        let phantom_data = if generics_with_bounds.params.is_empty() {
            TokenStream::new()
        } else {
            quote!((PhantomData<fn() -> (#(#gen_params,)*)>))
        };
        let phantom_data_new = if generics_with_bounds.params.is_empty() {
            TokenStream::new()
        } else {
            quote!(PhantomData::<fn() -> (#(#gen_params,)*)>)
        };
        let docs = if self.docs.is_empty() {
            None
        } else {
            Some(&self.docs)
        }
        .into_iter()
        .collect::<Vec<_>>();
        let server = format_ident!("{}Server", service);
        let async_client = format_ident!("{}AsyncClient", service);
        let blocking_client = format_ident!("{}BlockingClient", service);
        let handler = format_ident!("{}Handler", service);

        let imports = {
            let vis = &self.vis;
            quote!(
                #vis use #module::{
                    #service,
                    #async_client,
                    #blocking_client,
                    #server
                };
            )
        };

        let (request_variants, request_streaming): (Vec<_>, Vec<_>) = self
            .methods
            .iter()
            .map(|method| {
                let snake_name = method.name.to_string();
                let name = ident_ccase!(pascal, method.name);
                let fields: Vec<_> = method
                    .args
                    .iter()
                    .map(|pat| Field {
                        attrs: vec![],
                        vis: Visibility::Inherited,
                        mutability: FieldMutability::None,
                        ident: None,
                        colon_token: None,
                        ty: *pat.ty.clone(),
                    })
                    .collect();
                let streaming = matches!(method.ret, ReturnType::Streaming(_));
                let is_streaming = quote!(Self::#name(..) => #streaming);
                (
                    quote!(
                        #[serde(rename = #snake_name)]
                        #name(#(#fields),*)
                    ),
                    is_streaming
                )
            })
            .unzip();

        let response_variants = self.methods.iter().map(|method| {
            let snake_name = method.name.to_string();
            let name = ident_ccase!(pascal, method.name);
            let ret = match &method.ret {
                ReturnType::Simple(ty) | ReturnType::Streaming(ty) => ty.clone(),
            };
            quote!(
                #[serde(rename = #snake_name)]
                #name(#ret)
            )
        });
        let request_to_name = self.methods.iter().map(|method| {
            let name = method.name.to_string();
            let variant = ident_ccase!(pascal, method.name);
            quote!(Self::#variant(..) => Cow::Borrowed(#name))
        });
        let response_to_name = self.methods.iter().map(|method| {
            let name = method.name.to_string();
            let variant = ident_ccase!(pascal, method.name);
            quote!(Self::#variant(..) => #name)
        });

        let server_fns = self.methods.iter().map(|method| {
            let name = &method.name;
            let params = &method.args;
            let docs = &method.docs;
            let docs = quote! {
                #(#[doc = #docs])*
            };
            match &method.ret {
                ReturnType::Simple(ret) => {
                    quote! {
                        #docs
                        fn #name(&self #(,#params)*) -> impl Future<Output=#ret> + Send;
                    }
                }
                ReturnType::Streaming(ret) => {
                    quote! {
                        #docs
                        ///
                        /// This function runs for the entire lifetime of the stream, the stream to the client is ended when this function returns
                        ///
                        /// This function *must* not block, doing so may block other request handling, if you need to run blocking code, spawn a task and await it
                        fn #name<'a>(&'a self, sink: impl Sink<#ret, Error = StreamError> + Send + 'a #(,#params)*) -> impl Future<Output=()> + Send;
                    }
                }
            }
        });
        let (handle_arms, stream_handle_arms): (Vec<_>, Vec<_>) = self.methods.iter().map(|method| {
            let name = &method.name;
            let variant = ident_ccase!(pascal, method.name);
            let params = method.args.iter().map(|pat| &pat.pat).collect::<Vec<_>>();
            let handle = match &method.ret {
                ReturnType::Simple(_) => {
                    quote! {
                        Request::#variant(#(#params),*) => Response::#variant(self.0.#name(#(#params),*).await),
                    }
                }
                ReturnType::Streaming(_) => {
                    quote! {}
                }
            };
            let streaming_handle = match &method.ret {
                ReturnType::Simple(..) => {
                    quote! {}
                }
                ReturnType::Streaming(_) => {
                    quote! {
                        Request::#variant(#(#params),*) => {
                            let sink = sink.with(async |value| Result::<_, S::Error>::Ok(Response::#variant(value)));
                            self.0.#name(sink, #(#params),*).await;
                        },
                    }
                }
            };
            (handle, streaming_handle)
        }).unzip();

        let async_client_fns = self.client_fns(true, &generics);
        let blocking_client_fns = self.client_fns(false, &generics);

        let trait_rpc_str = trait_rpc.to_token_stream().to_string().replace(' ', "");
        let async_client_docs = [
            LitStr::new(" This is the async client for the service, it produces requests from method calls", Span::call_site()),
            LitStr::new(" (including chained method calls) and sends the requests with the given", Span::call_site()),
            LitStr::new(&format!(" [transport]({trait_rpc_str}::AsyncClient) before returning the response"), Span::call_site()),
            LitStr::new("", Span::call_site()),
            LitStr::new(" The return value is always wrapped in a result: `Result<T, _Client::Error>` where `T` is the service return value", Span::call_site()),
        ].into_iter();
        let blocking_client_docs = [
            LitStr::new(" This is the blocking client for the service, it produces requests from method calls", Span::call_site()),
            LitStr::new(" (including chained method calls) and sends the requests with the given", Span::call_site()),
            LitStr::new(&format!(" [transport]({trait_rpc_str}::AsyncClient) before returning the response"), Span::call_site()),
            LitStr::new("", Span::call_site()),
            LitStr::new(" The return value is always wrapped in a result: `Result<T, _Client::Error>` where `T` is the service return value", Span::call_site()),
        ].into_iter();

        let service_doc = format!(" This is the [Rpc]({trait_rpc_str}::Rpc) definition for this service");

        quote! {
            #[allow(unused_imports, reason = "These might not always be used, but they should be available in this module anyway")]
            #imports

            #[allow(unused_imports, reason = "These might not always be used, but it's easier to include always")]
            mod #module {
                use std::borrow::Cow;
                use super::*;
                use std::convert::Infallible;
                use std::marker::PhantomData;
                use #trait_rpc::{
                    client::{AsyncClient, BlockingClient, ResponseStream, StreamClient, WrongResponseType},
                    futures::sink::{Sink, SinkExt},
                    futures::stream::{Stream, StreamExt},
                    serde::{Deserialize, Serialize},
                    server::{Handler, IntoHandler},
                    Rpc, RpcWithServer,
                    server::StreamError
                };

                #(
                    #(#[doc = #docs])*
                    ///
                )*
                #[doc = #service_doc]
                pub struct #service #generics_with_bounds #phantom_data;

                impl #generics_with_bounds Rpc for #service <#(#gen_params),*> #(where #(#maybe_generics: Send + 'static),*)* {
                    type AsyncClient<_Client: AsyncClient<Self::Request, Self::Response>> = #async_client<_Client #(,#gen_params)*>;
                    type BlockingClient<_Client: BlockingClient<Self::Request, Self::Response>> = #blocking_client<_Client #(,#gen_params)*>;
                    type Request = Request #generics;
                    type Response = Response #generics;
                    fn async_client<_Client: AsyncClient<Request #generics, Response #generics>>(transport: _Client) -> #async_client<_Client #(,#gen_params)*> {
                        #async_client(transport, #phantom_data_new)
                    }
                    fn blocking_client<_Client: BlockingClient<Request #generics, Response #generics>>(transport: _Client) -> #blocking_client<_Client #(,#gen_params)*> {
                        #blocking_client(transport, #phantom_data_new)
                    }
                    fn service_name() -> &'static str {
                        stringify!(#service)
                    }
                }

                impl<Server: #server #generics #(, #gen_params_with_bounds)*> RpcWithServer<Server> for #service #generics where #(#gen_params: Send + 'static),* {
                    type Handler = #handler<Server #(, #gen_params)*>;
                    fn handler(server: Server) -> Self::Handler {
                        #handler(server, #phantom_data_new)
                    }
                }


                #[derive(Debug, Serialize, Deserialize)]
                #[serde(crate = #serde)]
                #[serde(tag = "method", content = "args")]
                pub enum Request #generics {
                    #(#request_variants,)*
                }

                impl #generics #trait_rpc::Request for Request #generics {
                    fn is_streaming_response(&self) -> bool {
                        match self {
                            #(#request_streaming),*
                        }
                    }
                    fn name(&self) -> Cow<'static, str> {
                        match self {
                            #(#request_to_name),*
                        }
                    }
                }

                #[derive(Debug, Serialize, Deserialize)]
                #[serde(crate = #serde)]
                #[serde(tag = "method", content = "result")]
                pub enum Response #generics {
                    #(#response_variants,)*
                }

                impl #generics Response #generics {
                    fn fn_name(&self) -> &'static str {
                        match self {
                            #(#response_to_name),*
                        }
                    }
                }

                #(
                    #(#[doc = #docs])*
                    ///
                )*
                /// This is the trait which is used by the server side in order to serve the client
                pub trait #server #generics: Send + Sync {
                    #(#server_fns)*
                }

                /// A [Handler](Handler) which handles requests/responses for a given service
                #[derive(Debug, Clone)]
                pub struct #handler<_Server #(,#gen_params)*>(_Server, #phantom_data);
                impl<_Server: #server #generics #(, #gen_params_with_bounds)*> Handler for #handler<_Server #(,#gen_params)*> where #(#gen_params: Send + 'static),* {
                    type Rpc = #service #generics;
                    async fn handle(&self, request: Request #generics) -> Response #generics {
                        match request {
                            #(#handle_arms)*
                            _ => panic!("This is a streaming method, must call handle_stream_response")
                        }
                    }
                    async fn handle_stream_response<'a, S: Sink<Response #generics, Error = StreamError> + Send + 'a>(
                        &'a self,
                        request: Request #generics,
                        sink: S,
                    ) {
                        match request {
                            #(#stream_handle_arms)*
                            _ => panic!("This is not a streaming method, must call handle")
                        }
                    }
                }

                #(
                    #(#[doc = #docs])*
                    ///
                )*
                #(#[doc = #async_client_docs])*
                #[derive(Debug, Copy, Clone)]
                pub struct #async_client<_Client #(,#gen_params_with_bounds)*>(_Client, #phantom_data);
                #[allow(clippy::future_not_send)]
                impl<_Client: AsyncClient<Request #generics, Response #generics> #(, #gen_params_with_bounds)*> #async_client<_Client #(,#gen_params)*> {
                    #(#async_client_fns)*
                }

                #(
                    #(#[doc = #docs])*
                    ///
                )*
                #(#[doc = #blocking_client_docs])*
                #[derive(Debug, Copy, Clone)]
                pub struct #blocking_client<_Client #(,#gen_params)*>(_Client, #phantom_data);
                impl<_Client: BlockingClient<Request #generics, Response #generics> #(, #gen_params)*> #blocking_client<_Client #(,#gen_params)*> {
                    #(#blocking_client_fns)*
                }
            }
        }
    }

    fn to_tokens(&self, tokens: &mut TokenStream) {
        tokens.extend(self.to_token_stream());
    }
}

impl Rpc {
    fn client_fns(&self, is_async: bool, generics: &TokenStream) -> impl Iterator<Item = TokenStream> {
        let await_ = if is_async {
            vec![quote!(.await)]
        } else {
            vec![]
        };
        let async_ = if is_async {
            vec![quote!(async)]
        } else {
            vec![]
        };
        self.methods.iter().map(move |method| {
            let name = &method.name;
            let name_str = name.to_string();
            let params = &method.args;
            let args = method.args.iter().map(|pat| &pat.pat);
            let variant = ident_ccase!(pascal, name);
            let docs = &method.docs;
            let docs = quote! {
                #(#[doc = #docs])*
            };
            match &method.ret {
                ReturnType::Simple(ret) => {
                    quote! {
                        #docs
                        pub #(#async_)* fn #name(&self #(, #params)*) -> Result<#ret, _Client::Error> {
                            match self.0.send(Request::#variant(#(#args),*))#(#await_)*? {
                                Response::#variant(value) => Ok(value),
                                other => Err(WrongResponseType::new(#name_str, other.fn_name()).into()),
                            }
                        }
                    }
                }
                ReturnType::Streaming(ret) => {
                    if is_async {
                        quote! {
                            #docs
                            pub async fn #name(&self #(, #params)*) -> Result<ResponseStream<#ret, _Client::Error>, _Client::Error> where _Client: StreamClient<Request #generics, Response #generics> {
                                let stream = self.0.send_streaming_response(Request::#variant(#(#args),*)).await?;
                                Ok(
                                     Box::new(stream
                                         .map(|value| {
                                             match value {
                                                 Ok(Response::#variant(value)) => Ok(value),
                                                 Ok(other) => {
                                                     Err(WrongResponseType::new(#name_str, other.fn_name()).into())
                                                 }
                                                 Err(error) => Err(error.into()),
                                             }
                                         })),

                                )
                            }
                        }
                    } else {
                        quote! {}
                    }
                }
            }
        })
    }
}
