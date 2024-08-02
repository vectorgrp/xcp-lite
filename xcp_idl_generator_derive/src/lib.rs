//TODO: Remove
#![allow(warnings)]

extern crate proc_macro;

mod translator;

use proc_macro::TokenStream;
use quote::{quote, ToTokens};
use syn::{parse_macro_input, Data, DeriveInput};
use translator::*;

#[proc_macro_derive(IdlGenerator)]
pub fn idl_generator_derive(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let data_type = &input.ident;

    let gen = match input.data {
        Data::Struct(data_struct) => {
            let field_handlers: Vec<_> = data_struct
                .fields
                .iter()
                .map(|field| {
                    //TODO Error handling
                    let field_name = &field.ident.as_ref().unwrap();
                    let field_type = &field.ty;

                    let f_name_str = field_name.to_string();
                    let f_type_str = field_type.into_token_stream().to_string();

                    //TODO: This is hardcoded to CDR here
                    let translated_t_type_str = CDR_TYPE_TRANSLATION
                        .get(f_type_str.as_str())
                        .unwrap()
                        .to_string();

                    //TODO: Remove redundant to_string?
                    quote! {
                        struct_fields.push(IdlStructField::new(
                            #f_name_str.to_string(),
                            #f_type_str.to_string()
                        ));
                    }

                })
                .collect();

            quote! {
                impl IdlGenerator for #data_type {
                    fn generate_idl() -> IdlStruct {
                        let mut struct_fields = IdlStructFieldVec::new();
                        #(#field_handlers)*
                        IdlStruct::new(stringify!(#data_type).to_owned(), struct_fields)
                    }
                }
            }
        }
        _ => panic!("IdlGenerator macro only supports structs"),
    };

    gen.into()
}
