// Derivation of `#[derive(McRegisterEnum)]` for integer enums.
//
// Applied to an enum definition, this reads the enum's `#[repr(<int>)]` (the backing integer
// type) and the variant names / discriminants, and generates an `impl McEnumType` carrying:
//   * the `McValueType` of the backing integer, and
//   * the A2L verbal-conversion unit string `<int> "<label>"` pairs (e.g. `0 "Off" 1 "On"`).
//
// A `#[characteristic(enum_type)]` field then looks these up via the trait instead of restating
// `enum_type = "u8"` and the `unit` string at every use site.

use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{Data, DeriveInput, Expr, Lit, UnOp};

use crate::ty::enum_int_value_type_tokens;

pub(crate) fn expand_enum(input: &DeriveInput) -> syn::Result<TokenStream2> {
    let enum_ident = &input.ident;

    let data = match &input.data {
        Data::Enum(e) => e,
        _ => {
            return Err(syn::Error::new_spanned(enum_ident, "McRegisterEnum can only be derived for enums"));
        }
    };

    // Backing integer type from `#[repr(<int>)]`.
    let repr_int = find_repr_int(input)?;
    let value_type = enum_int_value_type_tokens(&repr_int).ok_or_else(|| {
        syn::Error::new_spanned(
            enum_ident,
            format!("McRegisterEnum requires an integer `#[repr(..)]` (u8/u16/u32/u64/usize/i8/i16/i32/i64/isize), got `{repr_int}`"),
        )
    })?;

    // Build the A2L enum-format unit string from variant discriminants and names.
    let mut next_value: i128 = 0;
    let mut pairs: Vec<String> = Vec::new();
    for variant in &data.variants {
        if !matches!(variant.fields, syn::Fields::Unit) {
            return Err(syn::Error::new_spanned(&variant.ident, "McRegisterEnum only supports fieldless (unit) enum variants"));
        }
        let value = match &variant.discriminant {
            Some((_, expr)) => eval_discriminant(expr)?,
            None => next_value,
        };
        next_value = value + 1;
        pairs.push(format!("{value} \"{}\"", variant.ident));
    }
    let unit = pairs.join(" ");

    let expanded = quote! {
        impl ::xcp_registry::McEnumType for #enum_ident {
            fn mc_value_type() -> ::xcp_registry::McValueType {
                #value_type
            }
            fn mc_enum_unit() -> &'static str {
                #unit
            }
        }
    };

    Ok(expanded)
}

/// Find the integer name in a `#[repr(..)]` attribute (e.g. `u8` from `#[repr(u8)]`).
/// A non-integer repr (`C`, `transparent`, ...) or a missing repr is a compile error.
fn find_repr_int(input: &DeriveInput) -> syn::Result<String> {
    for attr in &input.attrs {
        if !attr.path().is_ident("repr") {
            continue;
        }
        let mut found: Option<String> = None;
        attr.parse_nested_meta(|meta| {
            if let Some(ident) = meta.path.get_ident() {
                let name = ident.to_string();
                if enum_int_value_type_tokens(&name).is_some() {
                    found = Some(name);
                }
            }
            Ok(())
        })?;
        if let Some(name) = found {
            return Ok(name);
        }
    }
    Err(syn::Error::new_spanned(
        &input.ident,
        "McRegisterEnum requires an explicit integer `#[repr(..)]`, e.g. `#[repr(u8)]`",
    ))
}

/// Evaluate an enum discriminant expression to an integer, supporting unary negation and
/// grouping. Non-literal discriminants (`SomeConst`, arithmetic) are rejected.
fn eval_discriminant(expr: &Expr) -> syn::Result<i128> {
    match expr {
        Expr::Lit(l) => match &l.lit {
            Lit::Int(i) => i.base10_parse::<i128>().map_err(|e| syn::Error::new_spanned(expr, e)),
            _ => Err(syn::Error::new_spanned(expr, "enum discriminant must be an integer literal")),
        },
        Expr::Unary(u) if matches!(u.op, UnOp::Neg(_)) => eval_discriminant(&u.expr).map(|v| -v),
        Expr::Group(g) => eval_discriminant(&g.expr),
        Expr::Paren(p) => eval_discriminant(&p.expr),
        _ => Err(syn::Error::new_spanned(expr, "McRegisterEnum only supports integer-literal enum discriminants")),
    }
}
