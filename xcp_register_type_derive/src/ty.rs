// Field type parsing and value-type token generation for the McRegisterType derive.

use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{Expr, Lit, Type};

/// Base (innermost, non-array) type of a field.
pub(crate) enum BaseType {
    /// A recognized scalar; carries the `McValueType` variant identifier name.
    Scalar(&'static str),
    /// A user-defined struct type; carries the full type path for recursion.
    User(Type),
}

/// Parsed field type: a base type plus up to two array dimensions.
pub(crate) struct FieldType {
    pub base: BaseType,
    pub x_dim: u16,
    pub y_dim: u16,
}

impl FieldType {
    /// Tokens for the `McValueType` of this field's base type.
    pub fn value_type_tokens(&self) -> TokenStream2 {
        match &self.base {
            BaseType::Scalar(variant) => {
                let ident = syn::Ident::new(variant, proc_macro2::Span::call_site());
                quote! { ::xcp_registry::McValueType::#ident }
            }
            BaseType::User(ty) => {
                let name = type_name(ty);
                quote! { ::xcp_registry::McValueType::new_typedef(#name) }
            }
        }
    }
}

/// Parse a field type into a `FieldType`.
///
/// Maps `[T; X]` to `x_dim = X, y_dim = 1` and `[[T; X]; Y]` to `x_dim = X, y_dim = Y`.
/// 3 or more dimensions is a compile error. No dimension folding.
pub(crate) fn parse_type(ty: &Type) -> syn::Result<FieldType> {
    // Collect array lengths from outer to inner.
    let mut dims: Vec<u16> = Vec::new();
    let mut cur = ty;
    while let Type::Array(arr) = cur {
        dims.push(parse_array_len(&arr.len)?);
        cur = &arr.elem;
    }

    if dims.len() > 2 {
        return Err(syn::Error::new_spanned(
            ty,
            "McRegisterType supports at most 2 array dimensions",
        ));
    }

    let (x_dim, y_dim) = match dims.len() {
        0 => (1u16, 1u16),
        1 => (dims[0], 1u16),
        // dims[0] is the outer (Y) length, dims[1] the inner (X) length.
        _ => (dims[1], dims[0]),
    };

    let base = parse_base(cur)?;
    Ok(FieldType { base, x_dim, y_dim })
}

fn parse_base(ty: &Type) -> syn::Result<BaseType> {
    if let Type::Path(tp) = ty {
        if tp.qself.is_none() {
            if let Some(seg) = tp.path.segments.last() {
                let ident = seg.ident.to_string();
                if let Some(variant) = scalar_variant(&ident) {
                    return Ok(BaseType::Scalar(variant));
                }
            }
        }
        return Ok(BaseType::User(ty.clone()));
    }
    Err(syn::Error::new_spanned(
        ty,
        "McRegisterType: unsupported field type (expected a scalar, an array, or a struct type)",
    ))
}

/// Map a Rust scalar type identifier to the `McValueType` variant name.
fn scalar_variant(s: &str) -> Option<&'static str> {
    Some(match s {
        "bool" => "Bool",
        "u8" => "Ubyte",
        "u16" => "Uword",
        "u32" => "Ulong",
        "u64" | "usize" => "Ulonglong",
        "i8" => "Sbyte",
        "i16" => "Sword",
        "i32" => "Slong",
        "i64" | "isize" => "Slonglong",
        "f32" => "Float32Ieee",
        "f64" => "Float64Ieee",
        _ => return None,
    })
}

/// The registry type name for a user-defined type: the last path segment identifier.
fn type_name(ty: &Type) -> String {
    if let Type::Path(tp) = ty {
        if let Some(seg) = tp.path.segments.last() {
            return seg.ident.to_string();
        }
    }
    // Fallback (should not happen for valid user types).
    quote! { #ty }.to_string()
}

fn parse_array_len(expr: &Expr) -> syn::Result<u16> {
    match expr {
        Expr::Lit(l) => {
            if let Lit::Int(i) = &l.lit {
                i.base10_parse::<u16>().map_err(|e| syn::Error::new_spanned(expr, e))
            } else {
                Err(syn::Error::new_spanned(expr, "array length must be an integer literal"))
            }
        }
        Expr::Paren(p) => parse_array_len(&p.expr),
        Expr::Group(g) => parse_array_len(&g.expr),
        _ => Err(syn::Error::new_spanned(expr, "array length must be an integer literal")),
    }
}
