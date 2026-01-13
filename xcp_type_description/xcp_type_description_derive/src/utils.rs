use syn::{Attribute, Lit, Meta, NestedMeta, Type, TypeArray};

#[derive(PartialEq)]
pub enum FieldAttribute {
    Undefined,
    Measurement,
    Characteristic,
    Axis,
}

impl FieldAttribute {
    pub fn to_str(&self) -> &'static str {
        match *self {
            FieldAttribute::Measurement => "measurement",
            FieldAttribute::Axis => "axis",
            FieldAttribute::Characteristic => "characteristic",
            FieldAttribute::Undefined => "undefined",
        }
    }
}

#[allow(clippy::type_complexity)]
pub fn parse_field_attributes(
    attributes: &Vec<Attribute>,
    _field_type: &Type,
) -> (
    FieldAttribute,
    String,
    String,
    Option<f64>,
    Option<f64>,
    Option<f64>,
    f64,
    f64,
    String,
    String,
    String,
    String,
    String,
) {
    // attribute
    let mut field_attribute: FieldAttribute = FieldAttribute::Undefined; // characteristic, axis, measurement

    // key-value pairs
    let mut qualifier = String::new();
    let mut comment = String::new();
    let mut factor: Option<f64> = Some(1.0);
    let mut offset: Option<f64> = Some(0.0);
    let mut min: Option<f64> = None;
    let mut max: Option<f64> = None;
    let mut step: Option<f64> = None;
    let mut unit = String::new();
    let mut x_axis = String::new();
    let mut y_axis = String::new();
    let mut x_axis_input_quantity = String::new();
    let mut y_axis_input_quantity = String::new();

    for attribute in attributes {
        //
        // Attributes not prefixed with characteristic, axis or typedef
        // are accepted but ignored as they likely belong to different derive macros
        if attribute.path.is_ident("characteristic") {
            field_attribute = FieldAttribute::Characteristic;
        } else if attribute.path.is_ident("axis") {
            field_attribute = FieldAttribute::Axis;
        } else if attribute.path.is_ident("measurement") {
            field_attribute = FieldAttribute::Measurement;
        } else {
            continue;
        }

        let meta_list = match attribute.parse_meta() {
            Ok(Meta::List(list)) => list,
            _ => panic!("Expected a list of attributes for type_description"),
        };

        for nested in meta_list.nested {
            let name_value = match nested {
                NestedMeta::Meta(Meta::NameValue(nv)) => nv,
                _ => panic!("Expected name-value pairs in type_description"),
            };

            let key = name_value.path.get_ident().unwrap_or_else(|| panic!("Expected identifier in type_description")).to_string();

            //TODO Figure out how to handle with Num after changing min,max,unit to range
            let value = match &name_value.lit {
                Lit::Str(s) => s.value(),
                _ => panic!("Expected string literal for key: {} in type_description", key),
            };

            match key.as_str() {
                "qualifier" => parse_str(&value, &mut qualifier),
                "comment" => parse_str(&value, &mut comment),
                "factor" => parse_f64(&value, &mut factor),
                "offset" => parse_f64(&value, &mut offset),
                "min" => parse_f64(&value, &mut min),
                "max" => parse_f64(&value, &mut max),
                "step" => parse_f64(&value, &mut step),
                "unit" => parse_str(&value, &mut unit),
                "x_axis" | "axis" => {
                    if field_attribute != FieldAttribute::Axis {
                        parse_str(&value, &mut x_axis)
                    }
                }
                "y_axis" => {
                    if field_attribute != FieldAttribute::Axis {
                        parse_str(&value, &mut y_axis)
                    }
                }
                "x_axis_inputQty" | "axis_inputQty" => {
                    if field_attribute != FieldAttribute::Axis {
                        parse_str(&value, &mut x_axis_input_quantity)
                    }
                }
                "y_axis_inputQty" => {
                    if field_attribute != FieldAttribute::Axis {
                        parse_str(&value, &mut y_axis_input_quantity)
                    }
                }
                _ => panic!("Unsupported type description attribute item: {}", key),
            }
        }
    }

    (
        field_attribute,
        qualifier,
        comment,
        min,
        max,
        step,
        factor.unwrap(),
        offset.unwrap(),
        unit,
        x_axis,
        y_axis,
        x_axis_input_quantity,
        y_axis_input_quantity,
    )
}

pub fn normalize_tokens(ts: proc_macro2::TokenStream) -> proc_macro2::TokenStream {
    ts.into_iter()
        .flat_map(|tt| match tt {
            proc_macro2::TokenTree::Group(g) if g.delimiter() == proc_macro2::Delimiter::None => normalize_tokens(g.stream()).into_iter().collect::<Vec<_>>(),
            other => vec![other],
        })
        .collect()
}

pub fn dimensions(ty: &syn::Type) -> (u16, u16) {
    match ty {
        syn::Type::Array(arr) => handle_array(arr),

        _ => (0, 0),
    }
}

fn handle_array(arr: &syn::TypeArray) -> (u16, u16) {
    let len = extract_array_len(&arr.len).unwrap_or(0);
    let (ix, iy) = dimensions(&arr.elem);
    if ix == 0 && iy == 0 {
        (len as u16, 0)
    } else if iy == 0 {
        (ix, len as u16)
    } else {
        (ix, iy * len as u16)
    }
}

fn extract_array_len(expr: &syn::Expr) -> Option<usize> {
    match expr {
        syn::Expr::Lit(l) => {
            if let syn::Lit::Int(i) = &l.lit {
                i.base10_parse().ok()
            } else {
                panic!("Expected an integer literal for array length");
            }
        }
        syn::Expr::Paren(p) => extract_array_len(&p.expr),
        syn::Expr::Group(g) => extract_array_len(&g.expr),
        _ => panic!("Expected an integer literal for array length"),
    }
}

#[inline]
fn parse_str(attribute: &str, string: &mut String) {
    *string = attribute.to_string();
}

#[inline]
fn parse_f64(attribute: &str, val: &mut Option<f64>) {
    *val = Some(attribute.parse::<f64>().expect("Failed to parse attribute text as f64"));
}
