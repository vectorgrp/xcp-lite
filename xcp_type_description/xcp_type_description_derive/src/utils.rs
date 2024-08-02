use syn::{Attribute, Lit, Meta, NestedMeta, Type, TypeArray, TypePath};

pub fn parse_characteristic_attributes(
    attributes: &Vec<Attribute>,
    field_type: &Type,
) -> (String, f64, f64, String) {
    let mut comment = String::new();
    let mut min: f64 = 0.0;
    let mut max: f64 = 0.0;
    let mut unit = String::new();

    let mut min_set: bool = false;
    let mut max_set: bool = false;

    for attribute in attributes {
        // Attributes not prefixed with type_description
        // are accepted but ignored as they likely
        // belong to different derived macros
        if !attribute.path.is_ident("type_description") {
            continue;
        }

        let meta_list = match attribute.parse_meta() {
            Ok(Meta::List(list)) => list,                                          // #[type_description(key = "This is correct)"]
            _ => panic!("Expected a list of attributes for type_description"),               // #[type_description = "This is incorrect"]
        };

        for nested in meta_list.nested {
            let name_value = match nested {
                NestedMeta::Meta(Meta::NameValue(nv)) => nv,                  // #[type_description(comment = "This is correct")]
                _ => panic!("Expected name-value pairs in type_description"),                // #[type_description(comment)] -> Incorrect
            };

            let key = name_value
                .path
                .get_ident()                                                  // #[type_description(comment = "This is correct")]
                .unwrap_or_else(|| panic!("Expected identifier in type_description")) // #[type_description("comment" = "This is incorrect")]
                .to_string();

            //TODO: Figure out how to handle with Num after changing min,max,unit to range
            let value = match &name_value.lit {
                Lit::Str(s) => s.value(),
                _ => panic!(
                    "Expected string literal for key: {} in type_description",
                    key
                ),
            };

            match key.as_str() {
                "comment" => parse_comment(&value, &mut comment),
                "min" => parse_min(&value, &mut min, &mut min_set),
                "max" => parse_max(&value, &mut max, &mut max_set),
                "unit" => parse_unit(&value, &mut unit),
                _ => panic!("Unsupported type description item: {}", key),
            }
        }
    }

    if !min_set {
        if let Some(min_val) = get_default_min_value_for_type(field_type) {
            min = min_val;
        }
    }

    if !max_set {
        if let Some(max_val) = get_default_max_value_for_type(field_type) {
            max = max_val;
        }
    }

    (comment, min, max, unit)
}

pub fn dimensions(ty: &Type) -> (usize, usize) {
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
                (length, 0)
            } else if inner_y == 0 {
                (length, inner_x)
            } else {
                (inner_x, inner_y)
            }
        }
        _ => (0, 0),
    }
}

#[inline]
fn parse_unit(attribute: &str, unit: &mut String) {
    *unit = attribute.to_string();
}

#[inline]
fn parse_comment(attribute: &str, comment: &mut String) {
    *comment = attribute.to_string()
}

#[inline]
fn parse_max(attribute: &str, max: &mut f64, max_set: &mut bool) {
    let parsed_max = attribute.parse::<f64>().expect("Failed to parse max");
    *max = parsed_max;
    *max_set = true;
}

#[inline]
fn parse_min(attribute: &str, min: &mut f64, min_set: &mut bool) {
    let parsed_min = attribute.parse::<f64>().expect("Failed to parse max");
    *min = parsed_min;
    *min_set = true;
}

fn get_default_min_value_for_type(ty: &Type) -> Option<f64> {
    match ty {
        Type::Path(TypePath { path, .. }) => {
            let segment = path.segments.last().expect("Expected a path segment");
            match segment.ident.to_string().as_str() {
                "bool" | "u8" | "u16" | "u32" | "u64" | "usize" => Some(0.0),
                "i8" => Some(i8::MIN as f64),
                "i16" => Some(i16::MIN as f64),
                "i32" => Some(i32::MIN as f64),
                "i64" | "isize" => Some(-1000000000000.0), //Some(i64::MIN as f64),
                "f32" => Some(-1000000000000.0),           //Some(f32::MIN as f64)
                "f64" => Some(-1000000000000.0),           //Some(f64::MIN)
                _ => None,
            }
        }
        _ => None,
    }
}

fn get_default_max_value_for_type(ty: &Type) -> Option<f64> {
    match ty {
        Type::Path(TypePath { path, .. }) => {
            let segment = path.segments.last().expect("Expected a path segment");
            match segment.ident.to_string().as_str() {
                "bool" => Some(255.0), //TODO: Discuss 255 vs 1 for bool values
                "u8" => Some(u8::MAX as f64),
                "u16" => Some(u16::MAX as f64),
                "u32" => Some(u32::MAX as f64),
                "usize" => Some(usize::MAX as f64),
                "i8" => Some(i8::MAX as f64),
                "i16" => Some(i16::MAX as f64),
                "i32" => Some(i32::MAX as f64),
                "isize" => Some(isize::MAX as f64),
                "u64" => Some(1000000000000.0), //Some(u64::MAX as f64),
                "i64" => Some(1000000000000.0), //Some(i64::MAX as f64),
                "f32" => Some(1000000000000.0), //Some(f32::MAX as f64),
                "f64" => Some(1000000000000.0), //Some(f64::MAX),
                _ => None,
            }
        }
        _ => None,
    }
}
