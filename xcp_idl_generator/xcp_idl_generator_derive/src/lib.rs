extern crate proc_macro;

use proc_macro::{Span, TokenStream};
use quote::{quote, ToTokens};
use syn::{parse_macro_input, Data, DeriveInput, Ident};

#[proc_macro_derive(IdlGenerator)]
pub fn idl_generator_derive(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let data_type = &input.ident;

    let gen = match input.data {
        Data::Struct(data_struct) => {
            let register_function_name = Ident::new(&format!("register_{}", data_type), Span::call_site().into());

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
                    fn description(&self) -> &'static Struct {
                        let structs = STRUCTS.lock().unwrap();
                        let struct_ref = structs.get(stringify!(#data_type)).unwrap();
                        struct_ref
                    }
                }

                #[ctor::ctor]
                fn #register_function_name() {
                    let mut struct_fields = FieldList::new();
                    #(#field_handlers)*

                    static mut STRUCT_INSTANCE: Option<Struct> = None;
                    static mut INITIALIZED: bool = false;

                    unsafe {
                        // Prevent the user from calling the register function multiple times
                        if INITIALIZED {
                            panic!("The register function has already been called.");
                        }

                        STRUCT_INSTANCE = Some(Struct::new(stringify!(#data_type), struct_fields));
                        let struct_ref = STRUCT_INSTANCE.as_ref().unwrap();
                        STRUCTS.lock().unwrap().insert(stringify!(#data_type), struct_ref);

                        INITIALIZED = true;
                    }
                }
            }
        }
        _ => panic!("IdlGenerator macro only supports structs"),
    };

    gen.into()
}
