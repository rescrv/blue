#![recursion_limit = "128"]

extern crate proc_macro;
#[macro_use]
extern crate quote;
extern crate syn;

use proc_macro2::TokenStream;
use quote::ToTokens;
use syn::{parse_macro_input, DeriveInput};

///////////////////////////////////// #[derive(TypedTupleKey)] /////////////////////////////////////

#[proc_macro_derive(TypedTupleKey, attributes(tuple_key))]
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
        syn::Fields::Named(fields) => {
            fields.named.iter().cloned().collect()
        },
        syn::Fields::Unnamed(_) => {
            panic!("unnamed structs are not supported");
        },
        syn::Fields::Unit => {
            panic!("unit structs are not supported");
        },
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
    // The prefix of the key must be of type message.
    let mut witnessed_non_message = false;
    for (idx, field) in fields.iter().enumerate() {
        if witnessed_non_message {
            panic!("invalid tuple-key: all but the last extension of the key must be type message");
        }
        let (num, ty) = parse_attributes(&field.attrs);
        let num = match num {
            Some(num) => num,
            None => {
                continue;
            },
        };
        if format!("{}", ty) != "message" {
            witnessed_non_message = true;
        }
        let ty = extract_type(ty);
        let field_name = &field.ident.as_ref().unwrap();
        let field_type = &field.ty;
        let line = if field_type.to_token_stream().to_string() == "()" {
            quote! {
                #sum
                match tkp.extend(::prototk::FieldNumber::must(#num), #ty) {
                    Ok(x) => x,
                    Err(e) => {
                        return Err(::tuple_key::Error::CouldNotExtend { field_number: #num, ty: #ty });
                    }
                }
                let #field_name = ();
            }
        } else {
            quote! {
                #sum
                let #field_name = match tkp.extend_with_key(::prototk::FieldNumber::must(#num), #ty) {
                    Ok(x) => x,
                    Err(e) => {
                        return Err(::tuple_key::Error::CouldNotExtend { field_number: #num, ty: #ty });
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
        let mut tkp: ::tuple_key::TupleKeyParser = ::tuple_key::TupleKeyParser::new(&tk);
        #sum
        Ok(#ty_name { #field_names })
    }
}

fn generate_into(fields: &[syn::Field]) -> TokenStream {
    let mut sum: TokenStream = quote! {};
    // The prefix of the key must be of type message.
    let mut witnessed_non_message = false;
    for field in fields.iter() {
        if witnessed_non_message {
            panic!("invalid tuple-key: all but the last extension of the key must be type message");
        }
        let (num, ty) = parse_attributes(&field.attrs);
        let num = match num {
            Some(num) => num,
            None => {
                continue;
            },
        };
        if format!("{}", ty) != "message" {
            witnessed_non_message = true;
        }
        let ty = extract_type(ty);
        let field_name = &field.ident.as_ref().unwrap();
        let field_type = &field.ty;
        let line = if field_type.to_token_stream().to_string() == "()" {
            quote! {
                #sum
                tk.extend(::prototk::FieldNumber::must(#num), #ty);
            }
        } else {
            quote! {
                #sum
                tk.extend_with_key(::prototk::FieldNumber::must(#num), self.#field_name, #ty);
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

fn extract_type(ty: TokenStream) -> TokenStream {
    let ty_str = format!("{}", ty);
    match ty_str.as_str() {
        "unit" => quote!{ ::tuple_key::DataType::Unit },
        "fixed32" => quote!{ ::tuple_key::DataType::Fixed32 },
        "fixed64" => quote!{ ::tuple_key::DataType::Fixed64 },
        "sfixed32" => quote!{ ::tuple_key::DataType::SFixed32 },
        "sfixed64" => quote!{ ::tuple_key::DataType::SFixed64 },
        "bytes" => quote!{ ::tuple_key::DataType::Bytes },
        "bytes16" => quote!{ ::tuple_key::DataType::Bytes16 },
        "bytes32" => quote!{ ::tuple_key::DataType::Bytes32 },
        "string" => quote!{ ::tuple_key::DataType::String },
        "message" => quote!{ ::tuple_key::DataType::Message },
        _ => {
            panic!("Don't know how to decode {}", ty_str);
        }
    }
}

//////////////////////////////////////////// attributes ////////////////////////////////////////////

const USAGE: &str = "must provide attributes of the form `tuple_key(field_number, field_type?)`";
const META_PATH: &str = "tuple_key";

fn parse_attribute(attr: &syn::Attribute) -> (Option<syn::LitInt>, TokenStream) {
    let meta = &attr.parse_meta().unwrap();
    if meta.path().clone().into_token_stream().to_string() != META_PATH {
        return (None, quote!{});
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
    if meta_list.nested.len() == 2 {
        let num = match &meta_list.nested[0] {
            syn::NestedMeta::Lit(syn::Lit::Int(field_number)) => {
                validate_field_number(field_number.base10_parse().unwrap());
                Some(field_number.clone())
            }
            _ => panic!("{}:{} {}", file!(), line!(), USAGE),
        };
        let ty = &meta_list.nested[1].to_token_stream();
        (num, ty.clone())
    } else {
        panic!("{}:{} {}", file!(), line!(), USAGE);
    }
}

fn parse_attributes(attrs: &[syn::Attribute]) -> (Option<syn::LitInt>, TokenStream) {
    for attr in attrs.iter() {
        if let (Some(field_number), token_stream) = parse_attribute(attr) {
            return (Some(field_number), token_stream);
        }
    }
    (None, quote! {})
}

////////////////////////////////////// protobuf field numbers //////////////////////////////////////

use prototk::FieldNumber;

fn validate_field_number(field_number: u32) {
    if let Err(err) = FieldNumber::new(field_number) {
        panic!("field_number={} number too invalid: {}", field_number, err);
    }
}
