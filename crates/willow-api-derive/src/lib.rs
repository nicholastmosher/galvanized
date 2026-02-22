use proc_macro::TokenStream;
use quote::quote;
use syn::{DeriveInput, parse_macro_input};

#[proc_macro_derive(WillowObject, attributes(willow))]
pub fn derive_willow_object(input: TokenStream) -> TokenStream {
    let _input = parse_macro_input!(input as DeriveInput);

    //
    quote! {}.into()
}
