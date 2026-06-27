// Field attribute parsing for the McRegisterType derive.
//
// Recognized classifiers: characteristic, axis, measurement (at most one per field).
// Numeric keys (min, max, step, factor, offset) take numeric literals (negative allowed).
// Text keys take string literals. Errors are reported as `compile_error!` with a span, never
// as panics.

use syn::{Expr, Field};

use crate::{expr_to_f64, expr_to_string};

/// Field classifier.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Classifier {
    None,
    Characteristic,
    Axis,
    Measurement,
}

/// Object qualifier.
#[derive(Debug, Clone, Copy)]
pub(crate) enum Qualifier {
    Volatile,
    ReadOnly,
}

/// Parsed attributes for a single field.
#[derive(Default)]
pub(crate) struct FieldAttrs {
    pub classifier: Classifier,
    pub comment: Option<String>,
    pub min: Option<f64>,
    pub max: Option<f64>,
    pub step: Option<f64>,
    pub factor: Option<f64>,
    pub offset: Option<f64>,
    pub unit: Option<String>,
    pub qualifier: Option<Qualifier>,
    pub axis: Option<String>,
    pub x_axis: Option<String>,
    pub y_axis: Option<String>,
    pub input_quantity: Option<String>,
    pub y_input_quantity: Option<String>,
}

impl Default for Classifier {
    fn default() -> Self {
        Classifier::None
    }
}

/// Parse all recognized attributes on a field.
pub(crate) fn parse_attrs(field: &Field) -> syn::Result<FieldAttrs> {
    let mut attrs = FieldAttrs::default();
    let mut classifier_set = false;

    for attr in &field.attrs {
        let classifier = if attr.path().is_ident("characteristic") {
            Classifier::Characteristic
        } else if attr.path().is_ident("axis") {
            Classifier::Axis
        } else if attr.path().is_ident("measurement") {
            Classifier::Measurement
        } else {
            // Unknown attribute: likely belongs to another derive macro. Ignore.
            continue;
        };

        if classifier_set {
            return Err(syn::Error::new_spanned(
                attr,
                "only one classifier attribute (characteristic / axis / measurement) is allowed per field",
            ));
        }
        classifier_set = true;
        attrs.classifier = classifier;

        attr.parse_nested_meta(|meta| {
            let key = meta
                .path
                .get_ident()
                .map(|i| i.to_string())
                .ok_or_else(|| meta.error("expected an attribute key identifier"))?;

            let value = meta.value()?;
            let expr: Expr = value.parse()?;

            apply_key(&mut attrs, classifier, &key, &expr, &meta)
        })?;
    }

    Ok(attrs)
}

fn apply_key(
    attrs: &mut FieldAttrs,
    classifier: Classifier,
    key: &str,
    expr: &Expr,
    meta: &syn::meta::ParseNestedMeta<'_>,
) -> syn::Result<()> {
    // Validate the key is allowed for the classifier.
    if !key_allowed(classifier, key) {
        return Err(meta.error(format!("`{key}` is not a valid key for this classifier")));
    }

    macro_rules! set_num {
        ($field:ident) => {{
            if attrs.$field.is_some() {
                return Err(meta.error(format!("duplicate key `{key}`")));
            }
            let v = expr_to_f64(expr).ok_or_else(|| meta.error(format!("`{key}` expects a numeric literal")))?;
            attrs.$field = Some(v);
        }};
    }
    macro_rules! set_str {
        ($field:ident) => {{
            if attrs.$field.is_some() {
                return Err(meta.error(format!("duplicate key `{key}`")));
            }
            let v = expr_to_string(expr).ok_or_else(|| meta.error(format!("`{key}` expects a string literal")))?;
            attrs.$field = Some(v);
        }};
    }

    match key {
        "comment" => set_str!(comment),
        "min" => set_num!(min),
        "max" => set_num!(max),
        "step" => set_num!(step),
        "factor" => set_num!(factor),
        "offset" => set_num!(offset),
        "unit" => set_str!(unit),
        "qualifier" => {
            if attrs.qualifier.is_some() {
                return Err(meta.error("duplicate key `qualifier`"));
            }
            let v = expr_to_string(expr).ok_or_else(|| meta.error("`qualifier` expects a string literal"))?;
            attrs.qualifier = Some(match v.as_str() {
                "volatile" => Qualifier::Volatile,
                "readonly" => Qualifier::ReadOnly,
                other => {
                    return Err(meta.error(format!("`qualifier` must be \"volatile\" or \"readonly\", got \"{other}\"")));
                }
            });
        }
        "axis" => set_str!(axis),
        "x_axis" => set_str!(x_axis),
        "y_axis" => set_str!(y_axis),
        "input_quantity" | "x_input_quantity" => set_str!(input_quantity),
        "y_input_quantity" => set_str!(y_input_quantity),
        _ => {
            return Err(meta.error(format!("unknown attribute key `{key}`")));
        }
    }

    Ok(())
}

/// Whether a key is valid for the given classifier.
///
/// `characteristic` allows all keys; `axis` and `measurement` allow only the scalar metadata
/// keys (no step, no axis references, no input quantities).
fn key_allowed(classifier: Classifier, key: &str) -> bool {
    let common = matches!(
        key,
        "comment" | "min" | "max" | "unit" | "factor" | "offset" | "qualifier"
    );
    match classifier {
        Classifier::Characteristic | Classifier::None => matches!(
            key,
            "comment"
                | "min"
                | "max"
                | "step"
                | "unit"
                | "factor"
                | "offset"
                | "qualifier"
                | "axis"
                | "x_axis"
                | "y_axis"
                | "input_quantity"
                | "x_input_quantity"
                | "y_input_quantity"
        ),
        Classifier::Axis | Classifier::Measurement => common,
    }
}
