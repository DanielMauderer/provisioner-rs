use proc_macro::TokenStream;

#[proc_macro_derive(Provision)]
pub fn derive_provision(input: TokenStream) -> TokenStream {
    let _ = input;
    TokenStream::new()
}
