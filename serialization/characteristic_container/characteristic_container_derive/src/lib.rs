extern crate proc_macro;

mod utils;

use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, Data, DeriveInput};
use utils::*;

// Characteristics have a predefined extension which is 0 for segment relative addressing mode
const XCP_ADDR_EXT_APP: u8 = 0;

#[proc_macro_derive(CharacteristicContainer, attributes(comment, min, max, unit))]
pub fn characteristic_container_derive(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let data_type = &input.ident;

    let gen = match input.data {
        Data::Struct(data_struct) => {
            generate_characteristics_container_impl(data_struct, data_type)
        }
        _ => panic!("CharacteristicContainer macro only supports structs"),
    };

    gen.into()
}

fn generate_characteristics_container_impl(
    data_struct: syn::DataStruct,
    data_type: &syn::Ident,
) -> proc_macro2::TokenStream {
    let field_handlers = data_struct.fields.iter().map(|field| {
        let field_name = &field.ident;
        let field_type = &field.ty;
        let field_attributes = &field.attrs;
        let (x_dim, y_dim) = dimensions(field_type);

        let (comment, min, max, unit) = parse_characteristic_attributes(
            field_attributes, field_type);

        quote! {
            // Offset is the address of the field relative to the address of the struct
            let offset = ((&self.#field_name as *const _ as *const u8 as usize) - (self as *const _ as *const u8 as usize)) as u16;

            // Check if the type of the field implements the CharacteristicContainer trait
            // If this is the case, the characteristic is a nested struct and its name must
            // be prefixed by the name of the parent. Consider the following:
            // struct Child { id: u32 }
            // struct Parent { child : Child } -> the name of Child.id characteristic should be Parent.Child.id
            if let Some(nested_characteristics) = <#field_type as CharacteristicContainer>::characteristics(&self.#field_name) {
                characteristics.extend(nested_characteristics.into_iter().map(|mut characteristic| {
                    characteristic.set_name(format!("{}.{}", stringify!(#data_type), characteristic.name()));
                    characteristic
                }));
            // If the type does not implement the CharacteristicContainer trait,
            // we can simply create a new Characteristic from it
            } else {
                // TODO: Initializing the calseg name with an empty string here is
                // a current design limitation. Ideally, characteristics should have
                // no knowledge of calibration segments
                characteristics.push(Characteristic::new(
                    "",
                    format!("{}.{}", stringify!(#data_type), stringify!(#field_name)),
                    stringify!(#field_type),
                    #comment,
                    #min,
                    #max,
                    #unit,
                    #x_dim,
                    #y_dim,
                    offset,
                    #XCP_ADDR_EXT_APP,
                ));
            }
        }
    });

    quote! {
        impl CharacteristicContainer for #data_type {
            fn characteristics(&self) -> Option<Vec<Characteristic>> {
                let mut characteristics = Vec::new();
                #(#field_handlers)*
                Some(characteristics)
            }
        }
    }
}
