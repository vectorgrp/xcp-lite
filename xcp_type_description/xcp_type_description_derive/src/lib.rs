extern crate proc_macro;

mod utils;

use proc_macro::TokenStream;
use quote::quote;
use syn::{Data, DeriveInput, Expr, parse_macro_input};

use utils::*;

// proc macro XcpTypeDescription
// With attributes measurement, characteristic, axis
// Example:
//  #[measurement(min = "0", max = "255")]
#[proc_macro_derive(XcpTypeDescription, attributes(measurement, characteristic, axis))]
pub fn xcp_type_description_derive(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let value_type = &input.ident;
    let generate = match input.data {
        Data::Struct(data_struct) => generate_type_description_impl(data_struct, value_type),
        _ => panic!("XcpTypeDescription macro only supports structs"),
    };
    generate.into()
}

fn generate_type_description_impl(data_struct: syn::DataStruct, value_type: &syn::Ident) -> proc_macro2::TokenStream {
    let field_handlers = data_struct.fields.iter().map(|field| {
        let field_name = &field.ident; // Field identifier
        let field_data_type = &field.ty; // Field type
        let (x_dim, y_dim) = dimensions(field_data_type); //Dimension of type is array

        // Field attributes
        // #[axis/characteristic/measurement(...)]  attr = access_type, comment, min, max, step, factor, offset, unit, x_axis, y_axis
        let field_attributes = &field.attrs;
        let (classifier, qualifier, comment, min, max, step, factor, offset, unit, x_axis_ref, y_axis_ref) = parse_field_attributes(field_attributes, field_data_type);
        let classifier_str = classifier.to_str(); // "characteristic" or "axis" or "undefined" from field attribute
        let min_token = syn::parse_str::<Expr>(format!("{:?}", min).as_str()).unwrap();
        let max_token = syn::parse_str::<Expr>(format!("{:?}", max).as_str()).unwrap();
        let step_token = syn::parse_str::<Expr>(format!("{:?}", step).as_str()).unwrap();

        quote! {
            // Offset is the address of the field relative to the address of the struct
            let addr_offset = std::mem::offset_of!(#value_type, #field_name) as u16;

            // Check if the type of the field implements the XcpTypeDescription trait
            // If this is the case, the type_description is a nested struct and its name must
            // be prefixed by the name of the parent. Consider the following:
            // struct Child { id: u32 }
            // struct Parent { child : Child } -> the name of Child.id type_description should be Parent.Child.id
            if let Some(inner_type_description) = <#field_data_type as XcpTypeDescription>::type_description(&self.#field_name,flat) {
                if flat {
                    type_description.extend(inner_type_description.into_iter().map(|mut inner| {
                        let mangled_name = Box::new(format!("{}.{}", stringify!(#field_name), inner.name()));
                        inner.set_name(Box::leak(mangled_name).as_str()); // Mangle names, leak the string, will be forever in the registry
                        inner.set_addr_offset(addr_offset + inner.addr_offset()); // Add offsets
                        inner
                    }));
                } else {
                    type_description.push(FieldDescriptor::new(
                        stringify!(#field_name), // Field identifier
                        Some(inner_type_description), // Inner StructDescriptor
                        stringify!(#field_data_type),
                        #classifier_str, // "characteristic", "measurement" or "axis" or "" from field attribute
                        #qualifier,
                        #comment,
                        #min_token,
                        #max_token,
                        #step_token,
                        #factor,
                        #offset,
                        #unit,
                        #x_dim,
                        #y_dim,
                        #x_axis_ref,
                        #y_axis_ref,
                        addr_offset,
                    ));
                }
            }
            // If the type does not implement the XcpTypeDescription trait, we can simply create a new FieldDescriptor from it
            else {
                //info!("Push type description for field {}: field_data_type={}[{}][{}]", stringify!(#field_name),stringify!(#field_data_type),#y_dim,#x_dim);
                type_description.push(FieldDescriptor::new(
                     stringify!(#field_name), // Field identifier
                    //format!("{}.{}", stringify!(#value_type), stringify!(#field_name)),
                    None,
                    stringify!(#field_data_type),
                    #classifier_str, // "characteristic", "measurement" or "axis" or "undefined" from field attribute
                    #qualifier,
                    #comment,
                    #min_token,
                    #max_token,
                    #step_token,
                    #factor,
                    #offset,
                    #unit,
                    #x_dim,
                    #y_dim,
                    #x_axis_ref,
                    #y_axis_ref,
                    addr_offset,
                ));
            }
        }
    });

    quote! {
        impl XcpTypeDescription for #value_type {
            fn type_description(&self, flat: bool) -> Option<StructDescriptor> {
                let mut type_description = StructDescriptor::new(stringify!(#value_type),std::mem::size_of::<#value_type>());
                #(#field_handlers)*
                Some(type_description)
            }
        }
    }
}
