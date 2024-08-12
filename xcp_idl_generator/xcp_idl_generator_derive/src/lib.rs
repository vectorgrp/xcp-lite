extern crate proc_macro;

use proc_macro::TokenStream;
use quote::{quote, ToTokens};
use syn::{parse_macro_input, Data, DeriveInput};

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
                    let f_type_str = f_type_str.replace(" ", "");

                    quote! {
                        struct_fields.push(Field::new(
                            #f_name_str,
                            #f_type_str
                        ));
                    }
                })
                .collect();

            quote! {
                impl IdlGenerator for #data_type {
                    fn description() -> Struct {
                        let mut struct_fields = FieldList::new();
                        #(#field_handlers)*
                        Struct::new(stringify!(#data_type), struct_fields)
                    }
                }
            }
        }
        _ => panic!("IdlGenerator macro only supports structs"),
    };

    gen.into()
}
