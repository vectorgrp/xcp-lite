// Crate xcp_register_type_derive
//
// Provides `#[derive(McRegisterType)]`. The derive generates an
// `impl ::xcp_registry::McRegisterType for T` whose `register` method calls the registry API
// directly (add_typedef / add_typedef_field / add_instance). No intermediate descriptor data
// structures are produced.
//
// The generated code emits fully-qualified `::xcp_registry::...` paths, so the consuming crate
// must depend on `xcp_registry` directly. This crate does NOT depend on `xcp_registry`.

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{Data, DeriveInput, Expr, Fields, Lit, UnOp, parse_macro_input};

mod attr;
mod ty;

use attr::{Classifier, FieldAttrs, Qualifier};
use ty::BaseType;

#[proc_macro_derive(McRegisterType, attributes(characteristic, axis, measurement))]
pub fn derive_mc_register_type(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    match expand(&input) {
        Ok(ts) => ts.into(),
        Err(err) => err.to_compile_error().into(),
    }
}

fn expand(input: &DeriveInput) -> syn::Result<TokenStream2> {
    let struct_ident = &input.ident;
    let type_name = struct_ident.to_string();
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    let fields = match &input.data {
        Data::Struct(s) => match &s.fields {
            Fields::Named(named) => &named.named,
            _ => {
                return Err(syn::Error::new_spanned(struct_ident, "McRegisterType only supports structs with named fields"));
            }
        },
        _ => {
            return Err(syn::Error::new_spanned(struct_ident, "McRegisterType only supports structs"));
        }
    };

    let mut nested_typedef_creation: Vec<TokenStream2> = Vec::new();
    let mut typedef_field_adds: Vec<TokenStream2> = Vec::new();
    let mut flatten_handling: Vec<TokenStream2> = Vec::new();

    for field in fields {
        let field_ident = field.ident.as_ref().expect("named field");
        let field_name = field_ident.to_string();

        let ft = ty::parse_type(&field.ty)?;
        let attrs = attr::parse_attrs(field)?;

        let value_type = ft.value_type_tokens();
        let x_dim = ft.x_dim;
        let y_dim = ft.y_dim;
        let obj = object_type_tokens(attrs.classifier);
        let support = build_support(&obj, &attrs);

        // Nested typedef creation (typedef mode): ensure the nested struct typedef exists first.
        if let BaseType::User(base_ty) = &ft.base {
            nested_typedef_creation.push(quote! {
                {
                    let __child = ctx.child_typedef();
                    <#base_ty as ::xcp_registry::McRegisterType>::register(&__child);
                }
            });
        }

        // Typedef field add.
        typedef_field_adds.push(quote! {
            {
                let __support = #support;
                let __dim = ::xcp_registry::McDimType::new(#value_type, #x_dim, #y_dim);
                let _ = ::xcp_registry::get_lock().as_mut().unwrap().add_typedef_field(
                    __TYPE_NAME,
                    #field_name,
                    __dim,
                    __support,
                    ::core::mem::offset_of!(#struct_ident, #field_ident) as u16,
                );
            }
        });

        // Flatten handling.
        match (&ft.base, ft.is_array()) {
            // Nested struct, scalar (no array dims): recurse and flatten its leaves.
            (BaseType::User(base_ty), false) => {
                flatten_handling.push(quote! {
                    {
                        let __child = ctx.child_flatten(
                            #field_name,
                            ::core::mem::offset_of!(#struct_ident, #field_ident) as u16,
                        );
                        <#base_ty as ::xcp_registry::McRegisterType>::register(&__child);
                    }
                });
            }
            // Array of nested struct. Two behaviors selected at runtime by
            // `ctx.flatten_struct_arrays`:
            //  - false: create the element typedef, then register one dimensioned typedef instance.
            //  - true:  flatten every element into indexed leaf instances (`field._i.leaf`), no typedef.
            (BaseType::User(base_ty), true) => {
                flatten_handling.push(quote! {
                    {
                        if ctx.flatten_struct_arrays {
                            let __field_off = ::core::mem::offset_of!(#struct_ident, #field_ident) as u16;
                            let __elem_size = ::core::mem::size_of::<#base_ty>() as u16;
                            let __count = (#x_dim as usize) * (#y_dim as usize);
                            for __i in 0..__count {
                                let __child = ctx.child_flatten_indexed(
                                    #field_name,
                                    __i,
                                    __field_off + (__i as u16) * __elem_size,
                                );
                                <#base_ty as ::xcp_registry::McRegisterType>::register(&__child);
                            }
                        } else {
                            {
                                let __child = ctx.child_typedef();
                                <#base_ty as ::xcp_registry::McRegisterType>::register(&__child);
                            }
                            let __name = format!("{}{}", ctx.name_prefix, #field_name);
                            let __support = #support;
                            let __dim = ::xcp_registry::McDimType::new(#value_type, #x_dim, #y_dim);
                            let __off = (ctx.addr_offset + ::core::mem::offset_of!(#struct_ident, #field_ident) as u16) as i32;
                            let _ = ::xcp_registry::get_lock().as_mut().unwrap().instance_list.add_instance(
                                __name, __dim, __support, ctx.target.address(__off),
                            );
                        }
                    }
                });
            }
            // Scalar or array of scalar: register as a leaf instance.
            (BaseType::Scalar(_), _) => {
                flatten_handling.push(quote! {
                    {
                        let __name = format!("{}{}", ctx.name_prefix, #field_name);
                        let __support = #support;
                        let __dim = ::xcp_registry::McDimType::new(#value_type, #x_dim, #y_dim);
                        let __off = (ctx.addr_offset + ::core::mem::offset_of!(#struct_ident, #field_ident) as u16) as i32;
                        let _ = ::xcp_registry::get_lock().as_mut().unwrap().instance_list.add_instance(
                            __name, __dim, __support, ctx.target.address(__off),
                        );
                    }
                });
            }
        }
    }

    let expanded = quote! {
        impl #impl_generics ::xcp_registry::McRegisterType for #struct_ident #ty_generics #where_clause {
            fn mc_type_name() -> &'static str {
                #type_name
            }

            fn register(ctx: &::xcp_registry::McRegisterContext) {
                const __TYPE_NAME: &str = #type_name;
                if !ctx.flatten {
                    // Create nested typedefs first so field references resolve.
                    #( #nested_typedef_creation )*
                    // Create this typedef.
                    let _ = ::xcp_registry::get_lock()
                        .as_mut()
                        .unwrap()
                        .add_typedef(__TYPE_NAME, ::core::mem::size_of::<#struct_ident>());
                    // Add all fields.
                    #( #typedef_field_adds )*
                    // Register one top-level instance referencing the typedef.
                    if ctx.level == 0 {
                        if let Some(__instance_name) = ctx.instance_name {
                            let __support = ::xcp_registry::McSupportData::new(ctx.target.default_object_type());
                            let _ = ::xcp_registry::get_lock().as_mut().unwrap().instance_list.add_instance(
                                __instance_name,
                                ::xcp_registry::McDimType::new(
                                    ::xcp_registry::McValueType::new_typedef(__TYPE_NAME),
                                    1,
                                    1,
                                ),
                                __support,
                                ctx.target.address(ctx.addr_offset as i32),
                            );
                        }
                    }
                } else {
                    #( #flatten_handling )*
                }
            }
        }
    };

    Ok(expanded)
}

/// Object type expression for a classifier. `None` defers to the runtime target default.
fn object_type_tokens(classifier: Classifier) -> TokenStream2 {
    match classifier {
        Classifier::Characteristic => quote! { ::xcp_registry::McObjectType::Characteristic },
        Classifier::Axis => quote! { ::xcp_registry::McObjectType::Axis },
        Classifier::Measurement => quote! { ::xcp_registry::McObjectType::Measurement },
        Classifier::None => quote! { ctx.target.default_object_type() },
    }
}

/// Build the `McSupportData` builder expression for a field.
fn build_support(obj: &TokenStream2, attrs: &FieldAttrs) -> TokenStream2 {
    let mut support = quote! { ::xcp_registry::McSupportData::new(#obj) };

    if let Some(comment) = &attrs.comment {
        support = quote! { #support.set_comment(#comment) };
    }
    if let Some(min) = attrs.min {
        support = quote! { #support.set_min(Some(#min)) };
    }
    if let Some(max) = attrs.max {
        support = quote! { #support.set_max(Some(#max)) };
    }
    if let Some(step) = attrs.step {
        support = quote! { #support.set_step(Some(#step)) };
    }
    if attrs.unit.is_some() || attrs.factor.is_some() || attrs.offset.is_some() {
        let factor = attrs.factor.unwrap_or(1.0);
        let offset = attrs.offset.unwrap_or(0.0);
        let unit = attrs.unit.clone().unwrap_or_default();
        support = quote! { #support.set_linear(#factor, #offset, #unit) };
    }
    if let Some(q) = attrs.qualifier {
        let qt = match q {
            Qualifier::Volatile => quote! { ::xcp_registry::McObjectQualifier::Volatile },
            Qualifier::ReadOnly => quote! { ::xcp_registry::McObjectQualifier::ReadOnly },
        };
        support = quote! { #support.set_qualifier(#qt) };
    }
    if let Some(a) = &attrs.axis {
        support = quote! { #support.set_x_axis_ref(Some(#a)) };
    }
    if let Some(a) = &attrs.x_axis {
        support = quote! { #support.set_x_axis_ref(Some(#a)) };
    }
    if let Some(a) = &attrs.y_axis {
        support = quote! { #support.set_y_axis_ref(Some(#a)) };
    }
    if let Some(a) = &attrs.input_quantity {
        support = quote! { #support.set_x_axis_input_quantity(Some(#a)) };
    }
    if let Some(a) = &attrs.y_input_quantity {
        support = quote! { #support.set_y_axis_input_quantity(Some(#a)) };
    }
    support
}

//----------------------------------------------------------------------------------------------
// Shared literal helpers (used by the attr module)

/// Evaluate an expression to f64, supporting unary negation and grouping.
pub(crate) fn expr_to_f64(expr: &Expr) -> Option<f64> {
    match expr {
        Expr::Lit(l) => lit_to_f64(&l.lit),
        Expr::Unary(u) if matches!(u.op, UnOp::Neg(_)) => expr_to_f64(&u.expr).map(|v| -v),
        Expr::Group(g) => expr_to_f64(&g.expr),
        Expr::Paren(p) => expr_to_f64(&p.expr),
        _ => None,
    }
}

fn lit_to_f64(lit: &Lit) -> Option<f64> {
    match lit {
        Lit::Int(i) => i.base10_parse::<f64>().ok(),
        Lit::Float(f) => f.base10_parse::<f64>().ok(),
        _ => None,
    }
}

/// Evaluate an expression to a string literal value.
pub(crate) fn expr_to_string(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Lit(l) => match &l.lit {
            Lit::Str(s) => Some(s.value()),
            _ => None,
        },
        Expr::Group(g) => expr_to_string(&g.expr),
        Expr::Paren(p) => expr_to_string(&p.expr),
        _ => None,
    }
}
