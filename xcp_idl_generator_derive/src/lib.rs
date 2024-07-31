extern crate proc_macro;

use proc_macro::TokenStream;
use syn::{parse_macro_input, DeriveInput};
use quote::quote;

#[proc_macro_derive(IdlGenerator)]
pub fn idl_generator_derive(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let data_type = &input.ident;

    let gen = quote! {
        impl IdlGenerator for #data_type {
            fn generate_idl(&self) {
                println!("Generating IDL")
            }
        }
    };

    gen.into()
}