use proc_macro2::TokenStream;
use quote::{quote, quote_spanned};
use syn::{
    Error, Field, Lit, Result, Token,
    ext::IdentExt,
    parse::{Parse, ParseStream},
    spanned::Spanned,
};

pub(crate) fn describe(field: &Field) -> Result<TokenStream> {
    let name = field
        .ident
        .as_ref()
        .ok_or_else(|| Error::new_spanned(field, "Reflect requires a named field"))?;
    let ty = &field.ty;
    let name = name.unraw().to_string();
    let mut output =
        quote_spanned! { ty.span() => ::mythraan_reflect::FieldInfo::new::<#ty>(#name) };

    let mut hidden = false;
    let mut range = None;
    for attribute in field
        .attrs
        .iter()
        .filter(|attribute| attribute.path().is_ident("reflect"))
    {
        attribute.parse_nested_meta(|meta| {
            if meta.path.is_ident("hidden") {
                if hidden {
                    return Err(meta.error("duplicate hidden attribute"));
                }
                hidden = true;
            } else if meta.path.is_ident("range") {
                if range.is_some() {
                    return Err(meta.error("duplicate range attribute"));
                }
                range = Some(meta.value()?.parse::<RangeHint>()?);
            } else {
                return Err(meta.error("expected hidden or range"));
            }
            Ok(())
        })?;
    }

    if hidden {
        output.extend(quote!(.with_hidden()));
    }
    if let Some(RangeHint {
        min,
        max,
        inclusive,
    }) = range
    {
        output.extend(
            quote!(.with_range(::mythraan_reflect::FieldRange::new(#min, #max, #inclusive))),
        );
    }
    Ok(output)
}

struct RangeHint {
    min: f64,
    max: f64,
    inclusive: bool,
}

impl Parse for RangeHint {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let min = read_bound(input)?;
        let limits = input.span();
        let inclusive = if input.peek(Token![..=]) {
            input.parse::<Token![..=]>()?;
            true
        } else {
            input.parse::<Token![..]>()?;
            false
        };
        let max = read_bound(input)?;
        if min > max || (!inclusive && min == max) {
            return Err(Error::new(limits, "reflection range must be nonempty"));
        }
        Ok(Self {
            min,
            max,
            inclusive,
        })
    }
}

fn read_bound(input: ParseStream<'_>) -> Result<f64> {
    let negative = input.peek(Token![-]);
    if negative {
        input.parse::<Token![-]>()?;
    }
    let literal: Lit = input.parse()?;
    let value: f64 = match &literal {
        Lit::Int(number) => number.base10_parse()?,
        Lit::Float(number) => number.base10_parse()?,
        _ => {
            return Err(Error::new_spanned(
                &literal,
                "expected a numeric range bound",
            ));
        }
    };
    if !value.is_finite() {
        return Err(Error::new_spanned(
            literal,
            "reflection range bound must be finite",
        ));
    }
    Ok(if negative { -value } else { value })
}
