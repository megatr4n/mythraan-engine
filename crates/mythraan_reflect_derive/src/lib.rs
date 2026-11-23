//! Derive macros for Mythraan's reflection API.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

use proc_macro::TokenStream;
use proc_macro2::TokenStream as Tokens;
use quote::quote;
use syn::{
    Attribute, Data, DeriveInput, Error, Fields, Meta, Result, Token, ext::IdentExt,
    parse_macro_input, punctuated::Punctuated,
};

mod field;

/// Generates reflection metadata for a named-field struct or unit-variant enum.
///
/// Named fields can use `#[reflect(hidden)]` and `#[reflect(range = -1.0..=1.0)]`.
#[proc_macro_derive(Reflect, attributes(reflect))]
pub fn derive_reflect(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    expand(input)
        .unwrap_or_else(Error::into_compile_error)
        .into()
}

fn expand(input: DeriveInput) -> Result<Tokens> {
    reject_misplaced_attributes(&input.attrs)?;
    let name = &input.ident;
    let mut access = Tokens::new();
    let (fields, variants) = match input.data {
        Data::Struct(data) => {
            let Fields::Named(fields) = data.fields else {
                return Err(Error::new_spanned(
                    name,
                    "Reflect requires a struct with named fields",
                ));
            };
            if !is_packed(&input.attrs) && !fields.named.is_empty() {
                let members: Vec<_> = fields
                    .named
                    .iter()
                    .filter_map(|field| field.ident.as_ref())
                    .collect();
                let names: Vec<_> = members
                    .iter()
                    .map(|ident| ident.unraw().to_string())
                    .collect();
                access = quote! {
                    fn field(&self, name: &str) -> ::core::option::Option<&dyn ::core::any::Any> {
                        match name {
                            #(#names => ::core::option::Option::Some(&self.#members),)*
                            _ => ::core::option::Option::None,
                        }
                    }

                    fn field_mut(&mut self, name: &str) -> ::core::option::Option<&mut dyn ::core::any::Any> {
                        match name {
                            #(#names => ::core::option::Option::Some(&mut self.#members),)*
                            _ => ::core::option::Option::None,
                        }
                    }
                };
            }
            let fields = fields
                .named
                .iter()
                .map(field::describe)
                .collect::<Result<Vec<_>>>()?;
            (fields, Vec::new())
        }
        Data::Enum(data) => {
            let variants = data
                .variants
                .iter()
                .map(|variant| {
                    reject_misplaced_attributes(&variant.attrs)?;
                    if !matches!(variant.fields, Fields::Unit) {
                        return Err(Error::new_spanned(
                            &variant.ident,
                            "Reflect requires enum variants without fields",
                        ));
                    }
                    Ok(variant.ident.unraw().to_string())
                })
                .collect::<Result<Vec<_>>>()?;
            (Vec::new(), variants)
        }
        Data::Union(_) => return Err(Error::new_spanned(name, "Reflect does not support unions")),
    };

    let mut generics = input.generics;
    generics
        .make_where_clause()
        .predicates
        .push(syn::parse_quote!(Self: 'static));
    let (impl_generics, type_generics, where_clause) = generics.split_for_impl();

    Ok(quote! {
        #[automatically_derived]
        impl #impl_generics ::mythraan_reflect::Reflect for #name #type_generics #where_clause {
            fn fields() -> &'static [::mythraan_reflect::FieldInfo] {
                const { &[#(#fields),*] }
            }

            fn variants() -> &'static [&'static str] {
                &[#(#variants),*]
            }
        }

        #[automatically_derived]
        impl #impl_generics ::mythraan_reflect::ReflectFields for #name #type_generics #where_clause {
            #access
        }
    })
}

fn is_packed(attributes: &[Attribute]) -> bool {
    attributes
        .iter()
        .filter(|attribute| attribute.path().is_ident("repr"))
        .filter_map(|attribute| {
            attribute
                .parse_args_with(Punctuated::<Meta, Token![,]>::parse_terminated)
                .ok()
        })
        .flatten()
        .any(|layout| layout.path().is_ident("packed"))
}

fn reject_misplaced_attributes(attributes: &[Attribute]) -> Result<()> {
    if let Some(attribute) = attributes
        .iter()
        .find(|attribute| attribute.path().is_ident("reflect"))
    {
        return Err(Error::new_spanned(
            attribute,
            "reflect attributes belong on named fields",
        ));
    }
    Ok(())
}
