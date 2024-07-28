use syn::{Attribute, Lit, Meta, Type, TypeArray, TypePath};

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
        let ident = attribute.path.get_ident().unwrap().to_string();
        match ident.as_str() {
            "comment" => parse_comment(attribute, &mut comment),
            "min" => parse_min(attribute, &mut min, &mut min_set),
            "max" => parse_max(attribute, &mut max, &mut max_set),
            "unit" => parse_unit(attribute, &mut unit),
            _ => continue,
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

fn parse_unit(attribute: &Attribute, unit: &mut String) {
    let meta = attribute.parse_meta().unwrap_or_else(|e| {
        panic!("Failed to parse 'unit' attribute: {}", e);
    });

    let unit_str = match meta {
        Meta::NameValue(meta) => match meta.lit {
            Lit::Str(unit_str) => unit_str,
            _ => panic!("Expected a string literal for 'unit'"),
        },
        _ => panic!("Expected 'unit' attribute to be a name-value pair"),
    };

    *unit = unit_str.value();
}

fn parse_max(attribute: &Attribute, max: &mut f64, max_set: &mut bool) {
    let meta = attribute.parse_meta().unwrap_or_else(|e| {
        panic!("Failed to parse 'max' attribute: {}", e);
    });

    let max_value = match meta {
        Meta::NameValue(meta) => match meta.lit {
            // NOTE: we are forced to limit the user to defining the min
            // and max attributes as strings instead of integers because
            // negative numbers are not interpreted as single literals in
            // Rust. This means # [max = 100] would work but #[max = -100]
            // would cause a compilation error
            Lit::Str(lit_str) => lit_str.value().parse::<f64>().unwrap(),
            _ => panic!("Expected a string literal for 'max'"),
        },
        _ => panic!("Expected 'max' attribute to be a name-value pair"),
    };

    *max = max_value;
    *max_set = true;
}

fn parse_min(attribute: &Attribute, min: &mut f64, min_set: &mut bool) {
    let meta = attribute.parse_meta().unwrap_or_else(|e| {
        panic!("Failed to parse 'min' attribute: {}", e);
    });

    let min_value = match meta {
        Meta::NameValue(meta) => match meta.lit {
            // NOTE: we are forced to limit the user to defining the min
            // and max attributes as strings instead of integers because
            // negative numbers are not interpreted as single literals in
            // Rust. This means # [min = 100] would work but #[min = -100]
            // would cause a compilation error
            Lit::Str(lit_str) => lit_str.value().parse::<f64>().unwrap(),
            _ => panic!("Expected a string literal for 'min'"),
        },
        _ => panic!("Expected 'min' attribute to be a name-value pair"),
    };

    *min = min_value;
    *min_set = true;
}

fn parse_comment(attribute: &Attribute, comment: &mut String) {
    let meta = attribute.parse_meta().unwrap_or_else(|e| {
        panic!("Failed to parse 'comment' attribute: {}", e);
    });

    let comment_str = match meta {
        Meta::NameValue(meta) => match meta.lit {
            Lit::Str(comment_str) => comment_str,
            _ => panic!("Expected a string literal for 'comment'"),
        },
        _ => panic!("Expected 'comment' attribute to be a name-value pair"),
    };

    *comment = comment_str.value();
}

//TODO: Discuss why the actual min values for certain types are not used
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

//TODO: Discuss why the actual max values for certain types are not used
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
