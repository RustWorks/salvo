extern crate proc_macro;

use proc_macro::TokenStream;
use quote::quote;

#[proc_macro_attribute]
pub fn fn_handler(_: TokenStream, input: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(input as syn::ItemFn);
    let attrs = &input.attrs;
    let vis = &input.vis;
    let sig = &input.sig;
    let body = &input.block;
    let name = &sig.ident;

    if sig.asyncness.is_none() {
        return syn::Error::new_spanned(sig.fn_token, "only async fn is supported")
            .to_compile_error()
            .into();
    }

    (quote! {
        #[allow(non_camel_case_types)]
        #vis struct #name;
        impl #name {
            #(#attrs)*
            #sig {
                #body
            }
        }
        #[async_trait]
        impl novel::Handler for #name {
            async fn handle(&self, sconf: Arc<ServerConfig>, req: &mut Request, depot: &mut Depot, resp: &mut Response) {
                Self::#name(sconf, req, depot, resp).await
            }
        }
    }).into()
}