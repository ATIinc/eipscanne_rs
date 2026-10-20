//! The derives of `eds_parser`'s two reading traits: `#[derive(FromEntry)]` reads a struct from the
//! fields of an EDS entry, and `#[derive(FromField)]` a `bilge` bitfield or enum from one field.

use proc_macro::TokenStream;
use quote::quote;
use syn::{Data, DataStruct, DeriveInput, Error, Fields, parse_macro_input};

/// Implements `eds_parser`'s `FromEntry` for a struct with named fields, each member read by its
/// own `FromEntry`: a value of one field from the next field, a `Vec` from the rest of the entry:
///
/// ```text
/// #[derive(FromEntry)]                    impl FromEntry for Assembly {
/// pub struct Assembly {                       fn from_entry(entry: &mut Entry) -> Result<Self> {
///     pub name: String,                           Ok(Assembly {
///     ...                                             name: FromEntry::from_entry(entry)?,
///     pub members: Vec<AssemblyMember>,               ...
/// }                                                   members: FromEntry::from_entry(entry)?,
///                                                 })
///                                             }
///                                         }
/// ```
#[proc_macro_derive(FromEntry)]
pub fn derive_from_entry(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;
    let Data::Struct(DataStruct {
        fields: Fields::Named(named),
        ..
    }) = &input.data
    else {
        return Error::new_spanned(name, "FromEntry reads a struct with named fields")
            .to_compile_error()
            .into();
    };

    let reads = named.named.iter().map(|field| {
        let field_name = &field.ident;
        quote! { #field_name: crate::fields::FromEntry::from_entry(entry)? }
    });

    quote! {
        impl crate::fields::FromEntry for #name {
            fn from_entry(entry: &mut crate::syntax::Entry) -> ::anyhow::Result<#name> {
                Ok(#name { #(#reads,)* })
            }
        }
    }
    .into()
}

/// Implements `eds_parser`'s `FromField` for a `bilge` bitfield or enum: the field read as the
/// type's backing integer (`u8` for `#[bitsize(8)]`), then converted with `FromBits`:
///
/// ```text
/// #[bitsize(8)]                       impl FromField for CipDataType {
/// #[derive(FromBits, FromField)]          fn from_field(text: &str) -> Result<Self> {
/// pub enum CipDataType { ... }                u8::from_field(text).map(CipDataType::from)
///                                         }
///                                     }
/// ```
#[proc_macro_derive(FromField)]
pub fn derive_from_field(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;
    quote! {
        impl crate::fields::FromField for #name {
            fn from_field(text: &str) -> ::anyhow::Result<#name> {
                let integer: <#name as ::bilge::Bitsized>::ArbitraryInt =
                    crate::fields::FromField::from_field(text)?;
                Ok(#name::from(integer))
            }
        }
    }
    .into()
}
