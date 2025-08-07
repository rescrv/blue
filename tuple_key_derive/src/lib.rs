#![doc = include_str!("../README.md")]
#![recursion_limit = "128"]

extern crate proc_macro;
#[macro_use]
extern crate quote;
extern crate syn;

use proc_macro2::TokenStream;
use quote::ToTokens;
use syn::{parse_macro_input, DeriveInput};

///////////////////////////////////// #[derive(TypedTupleKey)] /////////////////////////////////////

/// Derive a TypedTupleKey.
#[proc_macro_derive(TypedTupleKey, attributes(tuple_key, reverse))]
pub fn derive_typed_tuple_key(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    // `ty_name` holds the type's identifier.
    let ty_name = input.ident;
    // Break out for templating purposes.
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    let ds = match input.data {
        syn::Data::Struct(ref ds) => ds,
        syn::Data::Enum(_) => {
            panic!("enums are not supported");
        }
        syn::Data::Union(_) => {
            panic!("unions are not supported");
        }
    };

    let fields: Vec<syn::Field> = match &ds.fields {
        syn::Fields::Named(fields) => fields.named.iter().cloned().collect(),
        syn::Fields::Unnamed(_) => {
            panic!("unnamed structs are not supported");
        }
        syn::Fields::Unit => {
            panic!("unit structs are not supported");
        }
    };

    let try_from_snippet = generate_try_from(&ty_name, &fields);
    let into_snippet = generate_into(&fields);

    // Generate the whole implementation.
    let gen = quote! {
        impl #impl_generics TryFrom<::tuple_key::TupleKey> for #ty_name #ty_generics #where_clause {
            type Error = ::tuple_key::Error;

            fn try_from(tk: ::tuple_key::TupleKey) -> Result<Self, Self::Error> {
                #try_from_snippet
            }
        }

        impl #impl_generics Into<::tuple_key::TupleKey> for #ty_name #ty_generics #where_clause {
            fn into(self) -> ::tuple_key::TupleKey {
                #into_snippet
            }
        }

        impl #impl_generics ::tuple_key::TypedTupleKey for #ty_name #ty_generics #where_clause {
        }
    };
    gen.into()
}

fn generate_try_from(ty_name: &syn::Ident, fields: &[syn::Field]) -> TokenStream {
    let mut sum: TokenStream = quote! {};
    let mut field_names: TokenStream = quote! {};
    for (idx, field) in fields.iter().enumerate() {
        let (num, dir) = match parse_attributes(&field.attrs) {
            Some((num, dir)) => (num, dir),
            None => {
                continue;
            }
        };
        let field_name = &field.ident.as_ref().unwrap();
        let field_type = &field.ty;
        let line = if field_type.to_token_stream().to_string() == "()" {
            quote! {
                #sum
                match tkp.parse_next(::prototk::FieldNumber::must(#num), #dir) {
                    Ok(x) => x,
                    Err(e) => {
                        return Err(::tuple_key::Error::CouldNotExtend {
                            core: ::zerror_core::ErrorCore::default(),
                            field_number: #num,
                        });
                    }
                }
                let #field_name = ();
            }
        } else {
            quote! {
                #sum
                let #field_name = match tkp.parse_next_with_key(::prototk::FieldNumber::must(#num), #dir) {
                    Ok(x) => x,
                    Err(e) => {
                        return Err(::tuple_key::Error::CouldNotExtend {
                            core: ::zerror_core::ErrorCore::default(),
                            field_number: #num,
                        });
                    }
                };
            }
        };
        sum = line;
        if idx == 0 {
            field_names = quote! {
                #field_name
            };
        } else {
            field_names = quote! {
                #field_names, #field_name
            }
        }
    }
    quote! {
        use ::tuple_key::Element;
        let mut tkp: ::tuple_key::TupleKeyParser = ::tuple_key::TupleKeyParser::new(&tk);
        #sum
        Ok(#ty_name { #field_names })
    }
}

fn generate_into(fields: &[syn::Field]) -> TokenStream {
    let mut sum: TokenStream = quote! {};
    for field in fields.iter() {
        let (num, dir) = match parse_attributes(&field.attrs) {
            Some((num, dir)) => (num, dir),
            None => {
                continue;
            }
        };
        let field_name = &field.ident.as_ref().unwrap();
        let line = if field.ty.to_token_stream().to_string() == "()" {
            quote! {
                #sum
                tk.extend(::prototk::FieldNumber::must(#num));
            }
        } else {
            quote! {
                #sum
                tk.extend_with_key(::prototk::FieldNumber::must(#num), self.#field_name, #dir);
            }
        };
        sum = line;
    }
    quote! {
        let mut tk: ::tuple_key::TupleKey = ::tuple_key::TupleKey::default();
        #sum
        tk
    }
}

//////////////////////////////////////////// attributes ////////////////////////////////////////////

const USAGE: &str = "must provide attributes of the form `tuple_key(field_number, field_type?)`";

fn parse_field_number_attribute(attr: &syn::Attribute) -> Option<syn::LitInt> {
    let meta = &attr.parse_meta().unwrap();
    if meta.path().clone().into_token_stream().to_string() != "tuple_key" {
        return None;
    }
    let meta_list = match meta {
        syn::Meta::Path(_) => {
            panic!("{}:{} {}", file!(), line!(), USAGE);
        }
        syn::Meta::List(ref ml) => ml,
        syn::Meta::NameValue(_) => {
            panic!("{}:{} {}", file!(), line!(), USAGE);
        }
    };
    if meta_list.nested.len() == 1 {
        match &meta_list.nested[0] {
            syn::NestedMeta::Lit(syn::Lit::Int(field_number)) => {
                validate_field_number(field_number.base10_parse().unwrap());
                Some(field_number.clone())
            }
            _ => panic!("{}:{} {}", file!(), line!(), USAGE),
        }
    } else {
        panic!("{}:{} {}", file!(), line!(), USAGE);
    }
}

fn parse_reverse_attribute(attr: &syn::Attribute) -> Option<TokenStream> {
    let meta = &attr.parse_meta().unwrap();
    if meta.path().clone().into_token_stream().to_string() != "reverse" {
        return None;
    }
    match meta {
        syn::Meta::Path(_) => {
            // TODO(rescrv):  I assume all paths are #[reverse]
            Some(quote! { ::tuple_key::Direction::Reverse })
        }
        syn::Meta::List(_) => {
            panic!("{}:{} {}", file!(), line!(), USAGE);
        }
        syn::Meta::NameValue(_) => {
            panic!("{}:{} {}", file!(), line!(), USAGE);
        }
    }
}

fn parse_attributes(attrs: &[syn::Attribute]) -> Option<(syn::LitInt, TokenStream)> {
    let mut field_number = None;
    let mut direction = quote! { ::tuple_key::Direction::Forward };
    for attr in attrs.iter() {
        if let Some(f) = parse_field_number_attribute(attr) {
            field_number = Some(f);
        }
        if let Some(d) = parse_reverse_attribute(attr) {
            direction = d;
        }
    }
    field_number.map(|f| (f, direction))
}

////////////////////////////////////// protobuf field numbers //////////////////////////////////////

use prototk::FieldNumber;

fn validate_field_number(field_number: u32) {
    if let Err(err) = FieldNumber::new(field_number) {
        panic!("field_number={field_number} number too invalid: {err}");
    }
}
