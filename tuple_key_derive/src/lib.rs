#![recursion_limit = "128"]

extern crate proc_macro;
#[macro_use]
extern crate quote;
extern crate syn;

use proc_macro2::TokenStream;
use quote::ToTokens;
use syn::{parse_macro_input, DeriveInput};

use derive_util::StructVisitor;

//////////////////////////////////// #[derive(FromIntoTupleKey)] ////////////////////////////////

#[proc_macro_derive(FromIntoTupleKey, attributes(tuple_key))]
pub fn derive_from_into_tuple_key(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    // `ty_name` holds the type's identifier.
    let ty_name = input.ident;
    // Break out for templating purposes.
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    let data = match input.data {
        syn::Data::Struct(ref ds) => ds,
        syn::Data::Enum(_) => {
            panic!("enums are not supported");
        }
        syn::Data::Union(_) => {
            panic!("unions are not supported");
        }
    };

    fn extract_type(field: &syn::Field) -> (String, TokenStream) {
        let ty = (&field.ty).into_token_stream();
        let ty_str = format!("{}", ty.into_token_stream());
        let ty_tok = match ty_str.as_str() {
            "u32" => { quote! { ::tuple_key::DataType::Fixed32 }},
            "u64" => { quote! { ::tuple_key::DataType::Fixed64 }},
            "i32" => { quote! { ::tuple_key::DataType::SFixed32 }},
            "i64" => { quote! { ::tuple_key::DataType::SFixed64 }},
            "&[u8]" => { quote! { ::tuple_key::DataType::Bytes }},
            "[u8; 16]" => { quote! { ::tuple_key::DataType::Bytes16 }},
            "[u8; 32]" => { quote! { ::tuple_key::DataType::Bytes32 }},
            "String" => { quote! { ::tuple_key::DataType::String }},
            "()" => { quote! { ::tuple_key::DataType::Message }},
            _ => {
                panic!("Don't know how to decode {}", ty_str);
            }
        };
        (ty_str, ty_tok)
    }

    // Create into_tuple_key
    fn extract_into(_ty_name: &syn::Ident, _ds: &syn::DataStruct, unnamed: &syn::FieldsUnnamed) -> TokenStream {
        let mut sum: TokenStream = quote! {};
        for (idx, field) in unnamed.unnamed.iter().enumerate() {
            let (ty_str, ty_tok) = extract_type(field);
            let num = parse_attributes(&field.attrs);
            if ty_str == "()" {
                let line = quote! {
                    #sum
                    tk.extend(::prototk::FieldNumber::must(#num), #ty_tok);
                };
                sum = line;
            } else {
                let idx: syn::Index = idx.into();
                let line = quote! {
                    #sum
                    tk.extend_with_key(::prototk::FieldNumber::must(#num), self.#idx, #ty_tok);
                };
                sum = line;
            }
        }
        quote! {
            let mut tk: TupleKey = TupleKey::default();
            #sum
            tk
        }
    }
    let mut into_tuple_key = TupleKeyStructVisitor {
        f: extract_into,
    };
    let into_tuple_key = into_tuple_key.visit_struct(&ty_name, &data);

    // Create from_tuple_key
    fn extract_from(ty_name: &syn::Ident, _ds: &syn::DataStruct, unnamed: &syn::FieldsUnnamed) -> TokenStream {
        let mut sum: TokenStream = quote! {};
        let mut fields: TokenStream = quote! {};
        for (idx, field) in unnamed.unnamed.iter().enumerate() {
            let (ty_str, ty_tok) = extract_type(&field);
            let num = parse_attributes(&field.attrs);
            let field_num = syn::Ident::new(&format!("field_{}", idx), num.span());
            if ty_str == "()" {
                let line = quote! {
                    #sum
                    let #field_num: () = ();
                    match tkp.extend(::prototk::FieldNumber::must(#num), #ty_tok) {
                        Ok(x) => x,
                        Err(e) => {
                            return Err(::tuple_key::Error::CouldNotExtend { field_number: #idx as u32 });
                        }
                    };
                };
                sum = line;
            } else {
                let line = quote! {
                    #sum
                    let mut #field_num = match tkp.extend_with_key(::prototk::FieldNumber::must(#num), #ty_tok) {
                        Ok(x) => x,
                        Err(e) => {
                            return Err(::tuple_key::Error::CouldNotExtend { field_number: #num });
                        }
                    };
                };
                sum = line;
            }
            if idx == 0 {
                fields = quote! {
                    #field_num
                }
            } else {
                fields = quote! {
                    #fields, #field_num
                }
            }
        }
        quote! {
            let mut tkp: ::tuple_key::TupleKeyParser = ::tuple_key::TupleKeyParser::new(&tk);
            #sum
            Ok(#ty_name ( #fields ))
        }
    }
    let mut from_tuple_key = TupleKeyStructVisitor {
        f: extract_from,
    };
    let from_tuple_key = from_tuple_key.visit_struct(&ty_name, &data);

    // Generate the whole implementation.
    let gen = quote! {
        impl #impl_generics ::tuple_key::FromIntoTupleKey for #ty_name #ty_generics #where_clause {
            fn from_tuple_key(tk: &TupleKey) -> Result<Self, ::tuple_key::Error> {
                #from_tuple_key
            }

            fn into_tuple_key(self) -> TupleKey {
                #into_tuple_key
            }
        }
    };
    gen.into()
}

/////////////////////////////////////// TupleKeyStructVisitor //////////////////////////////////////

struct TupleKeyStructVisitor<O, F: Fn(&syn::Ident, &syn::DataStruct, &syn::FieldsUnnamed) -> O> {
    f: F,
}

impl<O, F: Fn(&syn::Ident, &syn::DataStruct, &syn::FieldsUnnamed) -> O> TupleKeyStructVisitor<O, F> {
}

impl<O, F: Fn(&syn::Ident, &syn::DataStruct, &syn::FieldsUnnamed) -> O> StructVisitor for TupleKeyStructVisitor<O, F> {
    type Output = O;

    fn visit_struct_unnamed_fields(
        &mut self,
        ty_name: &syn::Ident,
        ds: &syn::DataStruct,
        fields: &syn::FieldsUnnamed,
    ) -> Self::Output {
        (self.f)(ty_name, ds, fields)
    }

    fn visit_struct_unit(&mut self, ty_name: &syn::Ident, ds: &syn::DataStruct) -> Self::Output {
        let fields = syn::FieldsUnnamed {
            paren_token: syn::token::Paren(ty_name.span()),
            unnamed: syn::punctuated::Punctuated::new(),
        };
        (self.f)(ty_name, ds, &fields)
    }
}

//////////////////////////////////////////// attributes ////////////////////////////////////////////

const USAGE: &str = "must provide attributes of the form `tuple_key(field_number, field_type?)`";
const META_PATH: &str = "tuple_key";

fn parse_attribute(attr: &syn::Attribute) -> Option<syn::LitInt> {
    let meta = &attr.parse_meta().unwrap();
    if meta.path().clone().into_token_stream().to_string() != META_PATH {
        return None;
    }
    let meta_list = match meta {
        syn::Meta::Path(_) => {
            panic!("{}", USAGE);
        }
        syn::Meta::List(ref ml) => ml,
        syn::Meta::NameValue(_) => {
            panic!("{}", USAGE);
        }
    };
    if meta_list.nested.len() == 1 {
        match &meta_list.nested[0] {
            syn::NestedMeta::Lit(syn::Lit::Int(field_number)) => {
                validate_field_number(field_number.base10_parse().unwrap());
                Some(field_number.clone())
            }
            _ => panic!("{}", USAGE),
        }
    } else {
        panic!("{}", USAGE);
    }
}

fn parse_attributes(attrs: &[syn::Attribute]) -> syn::LitInt {
    for attr in attrs.iter() {
        if let Some(field_number) = parse_attribute(attr) {
            return field_number;
        }
    }
    panic!("{}", USAGE);
}

////////////////////////////////////// protobuf field numbers //////////////////////////////////////

use prototk::{
    FIRST_FIELD_NUMBER, LAST_FIELD_NUMBER, FIRST_RESERVED_FIELD_NUMBER, LAST_RESERVED_FIELD_NUMBER,
};

fn validate_field_number(field_number: u64) {
    if field_number > u32::max_value() as u64 {
        panic!(
            "field_number={} number too large:  must be less than {}",
            field_number, LAST_FIELD_NUMBER
        );
    }
    let field_number: u32 = field_number.try_into().unwrap();
    if field_number < FIRST_FIELD_NUMBER {
        panic!("field_number={} must be a positive integer", field_number);
    }
    if field_number > LAST_FIELD_NUMBER {
        panic!(
            "field_number={} number too large:  must be less than {}",
            field_number, LAST_FIELD_NUMBER
        );
    }
    if (FIRST_RESERVED_FIELD_NUMBER..=LAST_RESERVED_FIELD_NUMBER).contains(&field_number) {
        panic!(
            "field_number={} reserved: reserved range [{}, {}]",
            field_number, FIRST_RESERVED_FIELD_NUMBER, LAST_RESERVED_FIELD_NUMBER
        );
    }
}
