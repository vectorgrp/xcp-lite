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
) -> (FieldAttribute, String, String, Option<f64>, Option<f64>, Option<f64>, f64, f64, String, String, String) {
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
                _ => panic!("Unsupported type description attribute item: {}", key),
            }
        }
    }

    (field_attribute, qualifier, comment, min, max, step, factor.unwrap(), offset.unwrap(), unit, x_axis, y_axis)
}

pub fn dimensions(ty: &Type) -> (u16, u16) {
    match ty {
        Type::Array(TypeArray { elem, len, .. }) => {
            let length = match len {
                syn::Expr::Lit(expr_lit) => {
                    if let Lit::Int(lit_int) = &expr_lit.lit {
                        lit_int.base10_parse::<usize>().unwrap()
                    } else {
                        panic!("Expected an integer literal for array length");
                    }
                }
                _ => panic!("Expected an integer literal for array length"),
            };

            let (inner_x, inner_y) = dimensions(elem);

            if inner_x == 0 && inner_y == 0 {
                (length.try_into().unwrap(), 0)
            } else if inner_y == 0 {
                (inner_x, length.try_into().unwrap())
            } else {
                // @@@@ TODO ????
                (inner_x, inner_y)
            }
        }
        _ => (0, 0),
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
