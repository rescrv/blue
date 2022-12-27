#![recursion_limit = "128"]

extern crate proc_macro;
#[macro_use]
extern crate quote;
extern crate syn;

use proc_macro2::TokenStream;
use proc_macro2::TokenTree;
use quote::ToTokens;
use syn::spanned::Spanned;
use syn::{parse_macro_input,DeriveInput};

//////////////////////////////////////// #[derive(Message)] ////////////////////////////////////////

// NOTE(rescrv):  This was my first-ever macro.  It's deeply intertwined with the guts of the
// prototk library.  I'd like to someday separate the two, but all I really wanted was the syntax
// of the procedural macro for declaring the types.  I really, really hope someone comes along and
// upstages me on this without messing with the syntax much if any.
#[proc_macro_derive(Message, attributes(prototk, wrong))]
pub fn derive_message(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    // `ty_name` holds the type's identifier.
    let ty_name = input.ident;
    // Break out for templating purposes.
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    // Find the lifetime, adjusting impl_generics if necessary.
    let mut lifetime = find_lifetimes(&impl_generics, &ty_generics);
    let mut exp_impl_generics = impl_generics.clone().into_token_stream();
    if let None = lifetime {
        let prototk_lifetime = syn::LifetimeDef {
            attrs: Vec::new(),
            lifetime: syn::Lifetime::new("'prototk", proc_macro2::Span::call_site()),
            colon_token: None,
            bounds: syn::punctuated::Punctuated::new(),
        };
        let mut generics = input.generics.clone();
        for param in &mut generics.params {
            match param {
                syn::GenericParam::Lifetime(param) => {
                    param.bounds.push(prototk_lifetime.lifetime.clone());
                }
                syn::GenericParam::Type(param) => {
                    param.bounds.push(syn::TypeParamBound::Lifetime(
                        prototk_lifetime.lifetime.clone(),
                    ));
                }
                syn::GenericParam::Const(_) => {}
            }
        }
        generics.params = Some(syn::GenericParam::Lifetime(prototk_lifetime))
            .into_iter()
            .chain(generics.params)
            .collect();
        let (ig, _, _) = generics.split_for_impl();
        exp_impl_generics = ig.into_token_stream();
        lifetime = Some(quote!{'prototk});
    }
    // Generate the message code.
    let mut message = PackMessageVisitor::new(quote!{stream(writer)}.into());
    let message_stream = message.visit(&ty_name, &input.data);
    // Generate the pack code.
    let mut pack = PackMessageVisitor::new(quote!{pack_sz()}.into());
    let pack_reqd_bytes = pack.visit(&ty_name, &input.data);
    let mut pack = PackMessageVisitor::new(quote!{into_slice(buf);}.into());
    let pack_into_slice = pack.visit(&ty_name, &input.data);
    // Generate the unpack code.
    let mut unpack = UnpackMessageVisitor::default();
    let unpack = unpack.visit(&ty_name, &input.data);
    // Generate the whole implementation.
    let gen = quote! {
        impl #exp_impl_generics ::prototk::Message<#lifetime> for #ty_name #ty_generics #where_clause {
        }

        impl #impl_generics ::prototk::Packable for #ty_name #ty_generics #where_clause {
            fn pack_sz(&self) -> usize {
                use prototk::{FieldType,FieldTypeAssigner,Message,v64};
                #pack_reqd_bytes
            }

            fn pack(&self, buf: &mut [u8]) {
                use prototk::{FieldType,FieldTypeAssigner,Message,v64};
                #pack_into_slice
            }

            fn stream<W: std::io::Write>(&self, writer: &mut W) -> std::result::Result<usize, std::io::Error> {
                use prototk::{FieldType,FieldTypeAssigner,Message,v64};
                #message_stream
            }
        }

        impl #exp_impl_generics ::prototk::Unpackable<#lifetime> for #ty_name #ty_generics #where_clause {
            type Error = prototk::Error;

            fn unpack<'b>(buf: &'b [u8]) -> std::result::Result<(Self, &'b [u8]), prototk::Error>
                where
                    'b: #lifetime,
            {
                use prototk::{FieldType,FieldTypeAssigner,Message,v64};
                #unpack
            }
        }
    };
    gen.into()
}

fn find_lifetime_in_generics<T: ToTokens>(generics: &T) -> Option<TokenStream> {
    // TODO(rescrv): ICK!
    let ts: TokenStream = generics.into_token_stream();
    let ts: Vec<TokenTree> = ts.into_iter().collect();
    if ts.len() >= 4 {
        match (&ts[1], &ts[2]) {
            (TokenTree::Punct(x), TokenTree::Ident(ident)) => {
                let ident = ident.clone();
                if x.as_char() == '\'' {
                    Some(quote!{ #x #ident })
                } else {
                    None
                }
            },
            (_, _) => {
                None
            },
        }
    } else {
        None
    }
}

fn find_lifetimes(impl_generics: &syn::ImplGenerics, ty_generics: &syn::TypeGenerics) -> Option<TokenStream> {
    match find_lifetime_in_generics(ty_generics) {
        Some(x) => { return Some(x); }
        None => {
            match find_lifetime_in_generics(impl_generics) {
                Some(x) => { Some(x) }
                None => { None },
            }
        }
    }
}

/////////////////////////////////////////// StructVisitor //////////////////////////////////////////

trait StructVisitor: Sized {
    type Output;

    fn visit_struct(&mut self, ty_name: &syn::Ident, ds: &syn::DataStruct) -> Self::Output {
        match ds.fields {
            syn::Fields::Named(ref fields) => self.visit_struct_named_fields(ty_name, ds, fields),
            syn::Fields::Unnamed(ref fields) => {
                self.visit_struct_unnamed_fields(ty_name, ds, fields)
            }
            syn::Fields::Unit => self.visit_struct_unit(ty_name, ds),
        }
    }

    fn visit_struct_named_fields(
        &mut self,
        _ty_name: &syn::Ident,
        _ds: &syn::DataStruct,
        _fields: &syn::FieldsNamed,
    ) -> Self::Output {
        panic!("{}", "structs with named fields are not supported");
    }

    fn visit_struct_unnamed_fields(
        &mut self,
        _ty_name: &syn::Ident,
        _ds: &syn::DataStruct,
        _fields: &syn::FieldsUnnamed,
    ) -> Self::Output {
        panic!("{}", "structs with unnamed fields are not supported");
    }

    fn visit_struct_unit(&mut self, _ty_name: &syn::Ident, _ds: &syn::DataStruct) -> Self::Output {
        panic!("{}", "unit structs are not supported");
    }
}

//////////////////////////////////////////// EnumVisitor ///////////////////////////////////////////

trait EnumVisitor: Sized {
    type Output;
    type VariantOutput;

    fn visit_enum(&mut self, ty_name: &syn::Ident, de: &syn::DataEnum) -> Self::Output {
        let mut variants = Vec::new();
        for v in de.variants.iter() {
            variants.push(self.visit_enum_variant(ty_name, de, v));
        }
        self.combine_variants(ty_name, de, &variants)
    }

    fn combine_variants(
        &mut self,
        ty_name: &syn::Ident,
        de: &syn::DataEnum,
        variants: &[Self::VariantOutput],
    ) -> Self::Output;

    fn visit_enum_variant(
        &mut self,
        ty_name: &syn::Ident,
        de: &syn::DataEnum,
        variant: &syn::Variant,
    ) -> Self::VariantOutput {
        match variant.fields {
            syn::Fields::Named(ref fields) => {
                self.visit_enum_variant_named_fields(ty_name, de, variant, fields)
            }
            syn::Fields::Unnamed(ref fields) => {
                self.visit_enum_variant_unnamed_fields(ty_name, de, variant, fields)
            }
            syn::Fields::Unit => self.visit_enum_variant_unit(ty_name, de, variant),
        }
    }

    fn visit_enum_variant_named_fields(
        &mut self,
        _ty_name: &syn::Ident,
        _de: &syn::DataEnum,
        _variant: &syn::Variant,
        _fields: &syn::FieldsNamed,
    ) -> Self::VariantOutput {
        panic!("{}", "enum variants with named fields are not supported");
    }

    fn visit_enum_variant_unnamed_fields(
        &mut self,
        _ty_name: &syn::Ident,
        _de: &syn::DataEnum,
        _variant: &syn::Variant,
        _fields: &syn::FieldsUnnamed,
    ) -> Self::VariantOutput {
        panic!("{}", "enum variants with unnamed fields are not supported");
    }

    fn visit_enum_variant_unit(
        &mut self,
        _ty_name: &syn::Ident,
        _de: &syn::DataEnum,
        _variant: &syn::Variant,
    ) -> Self::VariantOutput {
        panic!("{}", "unit enum variants are not supported");
    }
}

////////////////////////////////////// protobuf field numbers //////////////////////////////////////

// NOTE(rescrv):  This information is also in the prototk library.  To avoid a dependency between
// the two, and to avoid forcing everyone to opt into the derive macros, the information is
// duplicated here.  It's also duplicated in the proto spec.  No sense changing the former unless
// there's also a way to fix the duplication with the latter.

const FIRST_FIELD_NUMBER: u64 = 1;
const LAST_FIELD_NUMBER: u64 = (1 << 29) - 1;

const FIRST_RESERVED_FIELD_NUMBER: u64 = 19000;
const LAST_RESERVED_FIELD_NUMBER: u64 = 19999;

fn validate_field_number(field_number: u64) {
    if field_number < FIRST_FIELD_NUMBER {
        panic!("field_number={} must be a positive integer", field_number);
    }
    if field_number > LAST_FIELD_NUMBER {
        panic!(
            "field_number={} number too large:  must be less than {}",
            field_number, LAST_FIELD_NUMBER
        );
    }
    if field_number >= FIRST_RESERVED_FIELD_NUMBER && field_number <= LAST_RESERVED_FIELD_NUMBER {
        panic!(
            "field_number={} reserved: reserved range [{}, {}]",
            field_number, FIRST_RESERVED_FIELD_NUMBER, LAST_RESERVED_FIELD_NUMBER
        );
    }
}

// TODO(rescrv):  tests that these panic

////////////////////////////////////////// ProtoTKVisitor //////////////////////////////////////////

const META_PATH: &'static str = "prototk";
const USAGE: &'static str = "macro helpers must take the form `prototk(field#, type)`";

trait ProtoTKVisitor: StructVisitor<Output=TokenStream> + EnumVisitor<Output=TokenStream, VariantOutput=TokenStream> {
    fn visit(&mut self, ty_name: &syn::Ident, data: &syn::Data) -> TokenStream {
        match data {
            syn::Data::Struct(ref ds) => self.visit_struct(ty_name, ds),
            syn::Data::Enum(de) => self.visit_enum(ty_name, de),
            syn::Data::Union(_) => {
                panic!("{}", "unions are not supported");
            },
        }
    }

    fn field_snippet(
        &mut self,
        ctor: &TokenStream,
        field: &syn::Field,
        field_ident: &TokenStream,
        field_number: &syn::LitInt,
        field_type: &TokenStream,
    ) -> TokenStream;

    fn struct_snippet(&mut self, ty_name: &syn::Ident, fields: &[TokenStream]) -> TokenStream;

    fn variant_snippet(
        &mut self,
        ctor: &TokenStream,
        variant: &syn::Variant,
        field_number: &syn::LitInt,
        field_type: &TokenStream,
    ) -> TokenStream;

    fn enum_snippet(&mut self, ty_name: &syn::Ident, variants: &[TokenStream]) -> TokenStream;
}

fn field_type_tokens(field: &syn::Field, field_type: &syn::Path) -> TokenStream {
    if field_type.is_ident(&syn::Ident::new("message", field_type.span())) {
        let ty = &field.ty;
        let ret = ToTokens::into_token_stream(&field_type);
        quote! {
            #ret::<#ty>
        }
    } else {
        ToTokens::into_token_stream(&field_type)
    }
}

fn visit_attribute<V: ProtoTKVisitor>(
    _ctor: &syn::Ident,
    field: &syn::Field,
    field_ident: &TokenStream,
    attr: &syn::Attribute,
    v: &mut V,
) -> Option<TokenStream> {
    let meta = &attr.parse_meta().unwrap();
    // TODO(rescrv): Ick.
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
    if meta_list.nested.len() != 2 {
        panic!("{}", USAGE);
    }
    match (&meta_list.nested[0], &meta_list.nested[1]) {
        (
            syn::NestedMeta::Lit(syn::Lit::Int(field_number)),
            syn::NestedMeta::Meta(syn::Meta::Path(field_type)),
        ) => {
            validate_field_number(field_number.base10_parse().unwrap());
            let field_type = &field_type_tokens(field, field_type);
            let ctor = quote! { ctor };
            Some(v.field_snippet(
                &ctor,
                field,
                field_ident,
                &field_number,
                &field_type,
            ))
        }
        _ => panic!("{}", USAGE),
    }
}

impl<V: ProtoTKVisitor> StructVisitor for V {
    type Output = TokenStream;

    fn visit_struct_named_fields(
        &mut self,
        ty_name: &syn::Ident,
        _ds: &syn::DataStruct,
        fields: &syn::FieldsNamed,
    ) -> TokenStream {
        let mut unrolled: Vec<TokenStream> = Vec::new();
        for ref field in fields.named.iter() {
            let field_ident = &ToTokens::into_token_stream(&field.ident);
            for ref attr in field.attrs.iter() {
                if let Some(x) = visit_attribute::<V>(ty_name, field, field_ident, attr, self) {
                    unrolled.push(x);
                }
            }
        }
        self.struct_snippet(ty_name, &unrolled)
    }

    fn visit_struct_unnamed_fields(
        &mut self,
        ty_name: &syn::Ident,
        _ds: &syn::DataStruct,
        fields: &syn::FieldsUnnamed,
    ) -> TokenStream {
        let mut unrolled = Vec::new();
        for (index, ref field) in fields.unnamed.iter().enumerate() {
            let field_ident = syn::LitInt::new(
                &format!("{}", index),
                proc_macro2::Span::call_site(),
            );
            let field_ident = &ToTokens::into_token_stream(&field_ident);
            for ref attr in field.attrs.iter() {
                if let Some(x) = visit_attribute::<V>(ty_name, field, field_ident, attr, self) {
                    unrolled.push(x);
                }
            }
        }
        self.struct_snippet(ty_name, &unrolled)
    }

    fn visit_struct_unit(&mut self, ty_name: &syn::Ident, _ds: &syn::DataStruct) -> TokenStream {
        self.struct_snippet(ty_name, &[])
    }
}

impl<V: ProtoTKVisitor> EnumVisitor for V {
    type Output = TokenStream;
    type VariantOutput = TokenStream;

    fn combine_variants(
        &mut self,
        ty_name: &syn::Ident,
        _de: &syn::DataEnum,
        variants: &[Self::VariantOutput],
    ) -> TokenStream {
        self.enum_snippet(ty_name, variants.into())
    }

    fn visit_enum_variant_unnamed_fields(
        &mut self,
        ty_name: &syn::Ident,
        _de: &syn::DataEnum,
        variant: &syn::Variant,
        fields: &syn::FieldsUnnamed,
    ) -> Self::VariantOutput {
        if fields.unnamed.len() != 1 {
            panic!("{}", USAGE);
        }
        let field = &fields.unnamed[0];
        for ref attr in variant.attrs.iter() {
            let meta = &attr.parse_meta().unwrap();
            // TODO(rescrv): Double Ick!
            if meta.path().clone().into_token_stream().to_string() != META_PATH {
                continue;
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
            if meta_list.nested.len() != 2 {
                match field.ident {
                    Some(ref x) => panic!(
                        "variant must take the form {}(T) with a single unnamed field of type T",
                        x
                    ),
                    None => panic!("{}", USAGE),
                }
            }
            match (&meta_list.nested[0], &meta_list.nested[1]) {
                (
                    syn::NestedMeta::Lit(syn::Lit::Int(field_number)),
                    syn::NestedMeta::Meta(syn::Meta::Path(field_type)),
                ) => {
                    validate_field_number(field_number.base10_parse().unwrap());
                    let field_type = &field_type_tokens(field, field_type);
                    let variant_ident = &variant.ident;
                    let ctor = quote! { #ty_name :: #variant_ident };
                    return self.variant_snippet(
                        &ctor,
                        variant,
                        &field_number,
                        &field_type,
                    );
                }
                _ => panic!("{}", USAGE),
            }
        }
        panic!("{}", USAGE);
    }
}

//////////////////////////////////////// PackMessageVisitor ////////////////////////////////////////

#[derive(Default)]
struct PackMessageVisitor {
    call: TokenStream,
}

impl PackMessageVisitor {
    fn new(call: TokenStream) -> Self {
        Self {
            call
        }
    }
}

impl ProtoTKVisitor for PackMessageVisitor {
    fn field_snippet(
        &mut self,
        _ty_name: &TokenStream,
        field: &syn::Field,
        field_ident: &TokenStream,
        field_number: &syn::LitInt,
        field_type: &TokenStream,
    ) -> TokenStream {
        quote_spanned! { field.span() =>
            let tag = prototk::Tag {
                field_number: prototk::FieldNumber::must(#field_number),
                wire_type: prototk::field_types::#field_type::WIRE_TYPE,
            };
            let fta = prototk::FieldTypePacker::new(
                tag,
                std::marker::PhantomData::<prototk::field_types::#field_type>{},
                &self.#field_ident);
            let pa = pa.pack(fta);
        }
    }

    fn struct_snippet(&mut self, _ty_name: &syn::Ident, fields: &[TokenStream]) -> TokenStream {
        let call = &self.call;
        quote! {
            let pa = prototk::stack_pack(());
            #(#fields;)*
            pa.#call
        }
    }

    fn variant_snippet(
        &mut self,
        ctor: &TokenStream,
        variant: &syn::Variant,
        field_number: &syn::LitInt,
        field_type: &TokenStream,
    ) -> TokenStream {
        let call = &self.call;
        quote_spanned! { variant.span() =>
            #ctor(v) => {
                let tag = prototk::Tag {
                    field_number: prototk::FieldNumber::must(#field_number),
                    wire_type: prototk::field_types::#field_type::WIRE_TYPE,
                };
                let fta = prototk::FieldTypePacker::new(
                    tag,
                    std::marker::PhantomData::<prototk::field_types::#field_type>{},
                    v);
                let pa = prototk::stack_pack(fta);
                pa.#call
            }
        }
    }

    fn enum_snippet(&mut self, _ty_name: &syn::Ident, variants: &[TokenStream]) -> TokenStream {
        quote! {
            match self {
                #(#variants,)*
            }
        }
    }
}

/////////////////////////////////////// UnpackMessageVisitor ///////////////////////////////////////

#[derive(Default)]
struct UnpackMessageVisitor { }

impl ProtoTKVisitor for UnpackMessageVisitor {
    fn field_snippet(
        &mut self,
        _ty_name: &TokenStream,
        field: &syn::Field,
        field_ident: &TokenStream,
        field_number: &syn::LitInt,
        field_type: &TokenStream,
    ) -> TokenStream {
        quote_spanned! { field.span() =>
            (#field_number, prototk::field_types::#field_type::WIRE_TYPE) => {
                // TODO(rescrv):  I'd perfer to have option to skip/ignore error
                let tmp: prototk::field_types::#field_type = up.unpack()?;
                ret.#field_ident.assign_field_type(tmp.into_native());
            }
        }
    }

    fn struct_snippet(&mut self, ty_name: &syn::Ident, fields: &[TokenStream]) -> TokenStream {
        quote! {
            let mut ret: #ty_name = #ty_name::default();
            let mut up = prototk::Unpacker::new(buf);
            while !up.is_empty() {
                let tag: prototk::Tag = up.unpack()?;
                let num: u32 = tag.field_number.into();
                match (num, tag.wire_type) {
                    #(#fields,)*
                    // TODO(rescrv):  I'd prefer to lift lifecycle management of fields to a
                    // higher level and deal with it there, but that will take some examples.
                    (_, _) => {}
                };
            }
            Ok((ret, up.remain()))
        }
    }

    fn variant_snippet(
        &mut self,
        ctor: &TokenStream,
        variant: &syn::Variant,
        field_number: &syn::LitInt,
        field_type: &TokenStream,
    ) -> TokenStream {
        quote_spanned! { variant.span() =>
            (#field_number, prototk::field_types::#field_type::WIRE_TYPE) => {
                let tmp: prototk::field_types::#field_type = up.unpack()?;
                Ok((#ctor(tmp.into_native()), up.remain()))
            }
        }
    }

    fn enum_snippet(&mut self, _ty_name: &syn::Ident, variants: &[TokenStream]) -> TokenStream {
        quote! {
            let mut up = prototk::Unpacker::new(buf);
            let tag: prototk::Tag = up.unpack()?;
            let num: u32 = tag.field_number.into();
            let wire_type: prototk::WireType = tag.wire_type;
            match (num, wire_type) {
                #(#variants,)*
                _ => {
                    // TODO(rescrv): production-ready blocker
                    unimplemented!("enum_snippet does not gracefully handle unknown variants");
                },
            }
        }
    }
}
