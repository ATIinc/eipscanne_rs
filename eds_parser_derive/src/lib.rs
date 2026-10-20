//! `#[derive(FromFields)]`: reads a struct from the fields of an EDS entry, one struct field per
//! entry field, in the order the struct declares them.

use proc_macro::TokenStream;
use quote::quote;
use syn::{Data, DataStruct, DeriveInput, Error, Fields, parse_macro_input};

/// Implements `eds_parser`'s `FromFields` for a struct with named fields. Each field reads the
/// next entry field with `fields.next()?`, and a field marked `#[rest]` reads the rest of the
/// entry with `fields.rest()?`:
///
/// ```text
/// #[derive(FromFields)]                       impl FromFields for Assembly {
/// pub struct Assembly {                           fn from_fields(fields: &mut Fields) -> Result<Self> {
///     pub name: Option<String>,                       Ok(Assembly {
///     ...                                                 name: fields.next()?,
///     #[rest]                                             ...
///     pub members: Vec<AssemblyMember>,                   members: fields.rest()?,
/// }                                                   })
///                                                 }
///                                             }
/// ```
#[proc_macro_derive(FromFields, attributes(rest))]
pub fn derive_from_fields(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;
    let Data::Struct(DataStruct {
        fields: Fields::Named(named),
        ..
    }) = &input.data
    else {
        return Error::new_spanned(name, "FromFields reads a struct with named fields")
            .to_compile_error()
            .into();
    };

    let reads = named.named.iter().map(|field| {
        let field_name = &field.ident;
        let rest = field.attrs.iter().any(|attr| attr.path().is_ident("rest"));
        match rest {
            true => quote! { #field_name: fields.rest()? },
            false => quote! { #field_name: fields.next()? },
        }
    });

    quote! {
        impl crate::fields::FromFields for #name {
            fn from_fields(fields: &mut crate::fields::Fields) -> ::anyhow::Result<#name> {
                Ok(#name { #(#reads,)* })
            }
        }
    }
    .into()
}
