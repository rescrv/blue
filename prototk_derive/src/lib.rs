#![recursion_limit = "128"]
#![doc = include_str!("../README.md")]

extern crate proc_macro;
#[macro_use]
extern crate quote;
extern crate syn;

use proc_macro2::TokenStream;
use proc_macro2::TokenTree;
use quote::ToTokens;
use syn::spanned::Spanned;
use syn::{parse_macro_input, DeriveInput};

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
    if lifetime.is_none() {
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
        lifetime = Some(quote! {'prototk});
    }
    // Generate the message code.
    let mut message = PackMessageVisitor::new(quote! {stream(writer)});
    let message_stream = message.visit(&ty_name, &input.data);
    // Generate the pack code.
    let mut pack = PackMessageVisitor::new(quote! {pack_sz()});
    let pack_reqd_bytes = pack.visit(&ty_name, &input.data);
    let mut pack = PackMessageVisitor::new(quote! {into_slice(buf);});
    let pack_into_slice = pack.visit(&ty_name, &input.data);
    // Generate the unpack code.
    let mut unpack = UnpackMessageVisitor::default();
    let unpack = unpack.visit(&ty_name, &input.data);
    // Generate the whole implementation.
    let gen = quote! {
        impl #exp_impl_generics ::prototk::Message<#lifetime> for #ty_name #ty_generics #where_clause {
        }

        impl #impl_generics buffertk::Packable for #ty_name #ty_generics #where_clause {
            fn pack_sz(&self) -> usize {
                use buffertk::v64;
                use prototk::{FieldPacker, FieldPackHelper, FieldType, Message};
                #pack_reqd_bytes
            }

            fn pack(&self, buf: &mut [u8]) {
                use buffertk::v64;
                use prototk::{FieldPacker, FieldPackHelper, FieldType, Message};
                #pack_into_slice
            }

            fn stream<W: std::io::Write>(&self, writer: &mut W) -> std::result::Result<usize, std::io::Error> {
                use buffertk::v64;
                use prototk::{FieldPacker, FieldPackHelper, FieldType, Message};
                #message_stream
            }
        }

        impl #exp_impl_generics buffertk::Unpackable<#lifetime> for #ty_name #ty_generics #where_clause {
            type Error = ::prototk::Error;

            fn unpack<'b>(buf: &'b [u8]) -> std::result::Result<(Self, &'b [u8]), ::prototk::Error>
                where
                    'b: #lifetime,
            {
                use buffertk::{v64, Unpackable};
                use prototk::{FieldUnpackHelper, FieldType, Message};
                #unpack
            }
        }

        impl #exp_impl_generics ::prototk::FieldPackHelper<#lifetime, ::prototk::field_types::message<#ty_name #ty_generics>> for #ty_name #ty_generics #where_clause {
            fn field_pack_sz(&self, tag: &::prototk::Tag) -> usize {
                use buffertk::{stack_pack, Packable};
                use prototk::{FieldPackHelper, FieldType, Message};
                // TODO(rescrv):  Double stack-pack is double wasteful.
                stack_pack(tag).pack(stack_pack(self).length_prefixed()).pack_sz()
            }

            fn field_pack(&self, tag: &::prototk::Tag, out: &mut [u8]) {
                use buffertk::{stack_pack, Packable};
                use prototk::{FieldPackHelper, FieldType, Message};
                stack_pack(tag).pack(stack_pack(self).length_prefixed()).into_slice(out);
            }
        }

        impl #exp_impl_generics ::prototk::FieldUnpackHelper<#lifetime, ::prototk::field_types::message<#ty_name #ty_generics>> for #ty_name #ty_generics #where_clause {
            fn merge_field(&mut self, proto: ::prototk::field_types::message<#ty_name #ty_generics>) {
                *self = proto.unwrap_message();
            }
        }

        impl #impl_generics From<::prototk::field_types::message<Self>> for #ty_name #ty_generics #where_clause {
            fn from(proto: ::prototk::field_types::message<Self>) -> Self {
                proto.unwrap_message()
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
                    Some(quote! { #x #ident })
                } else {
                    None
                }
            }
            (_, _) => None,
        }
    } else {
        None
    }
}

fn find_lifetimes(
    impl_generics: &syn::ImplGenerics,
    ty_generics: &syn::TypeGenerics,
) -> Option<TokenStream> {
    match find_lifetime_in_generics(ty_generics) {
        Some(x) => Some(x),
        None => find_lifetime_in_generics(impl_generics),
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
    if (FIRST_RESERVED_FIELD_NUMBER..=LAST_RESERVED_FIELD_NUMBER).contains(&field_number) {
        panic!(
            "field_number={} reserved: reserved range [{}, {}]",
            field_number, FIRST_RESERVED_FIELD_NUMBER, LAST_RESERVED_FIELD_NUMBER
        );
    }
}

// TODO(rescrv):  tests that these panic

/////////////////////////////////////////////// USAGE //////////////////////////////////////////////

const USAGE: &str = "must provide attributes of the form `prototk(field_number, field_type)`";

////////////////////////////////////// meta path manipulation //////////////////////////////////////

const META_PATH: &str = "prototk";

fn parse_attribute(attr: &syn::Attribute) -> Option<(syn::LitInt, syn::Path)> {
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
    if meta_list.nested.len() != 2 {
        panic!("{}", USAGE);
    }
    match (&meta_list.nested[0], &meta_list.nested[1]) {
        (
            syn::NestedMeta::Lit(syn::Lit::Int(field_number)),
            syn::NestedMeta::Meta(syn::Meta::Path(field_type)),
        ) => {
            validate_field_number(field_number.base10_parse().unwrap());
            Some((field_number.clone(), field_type.clone()))
        }
        _ => panic!("{}", USAGE),
    }
}

fn parse_attributes(attrs: &[syn::Attribute]) -> (syn::LitInt, syn::Path) {
    for attr in attrs.iter() {
        if let Some((field_number, field_type)) = parse_attribute(attr) {
            return (field_number, field_type);
        }
    }
    panic!("{}", USAGE);
}

////////////////////////////////////////// ProtoTKVisitor //////////////////////////////////////////

trait ProtoTKVisitor:
    StructVisitor<Output = TokenStream> + EnumVisitor<Output = TokenStream, VariantOutput = TokenStream>
{
    fn visit(&mut self, ty_name: &syn::Ident, data: &syn::Data) -> TokenStream {
        match data {
            syn::Data::Struct(ref ds) => self.visit_struct(ty_name, ds),
            syn::Data::Enum(de) => self.visit_enum(ty_name, de),
            syn::Data::Union(_) => {
                panic!("{}", "unions are not supported");
            }
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

    fn named_variant_snippet(
        &mut self,
        ctor: &TokenStream,
        variant: &syn::Variant,
        field_number: &syn::LitInt,
        field_type: &syn::Path,
        fields: &syn::FieldsNamed,
    ) -> TokenStream;

    fn unnamed_variant_snippet(
        &mut self,
        ctor: &TokenStream,
        variant: &syn::Variant,
        field_number: &syn::LitInt,
        field_type: &TokenStream,
    ) -> TokenStream;

    fn unit_variant_snippet(
        &mut self,
        ctor: &TokenStream,
        variant: &syn::Variant,
        field_number: &syn::LitInt,
    ) -> TokenStream;

    fn enum_snippet(&mut self, ty_name: &syn::Ident, variants: &[TokenStream]) -> TokenStream;
}

fn field_type_tokens(field: &syn::Field, field_type: &syn::Path) -> TokenStream {
    if field_type.is_ident(&syn::Ident::new("message", field_type.span())) {
        let ret = ToTokens::into_token_stream(field_type);
        let ty = &field.ty;
        if let syn::Type::Path(path) = ty {
            if !path.path.segments.is_empty()
                && (path.path.segments[0].ident == syn::Ident::new("Vec", field_type.span())
                    || path.path.segments[0].ident == syn::Ident::new("Option", field_type.span()))
            {
                let tokens = ToTokens::into_token_stream(ty);
                let mut tokens: Vec<_> = tokens.into_token_stream().into_iter().collect();
                if tokens.len() < 3 {
                    panic!("unhandled case in prototk_derive: please file a bug report");
                }
                tokens.remove(0);
                tokens.remove(0);
                tokens.remove(tokens.len() - 1);
                let mut inner_type = quote! {};
                for token in tokens.into_iter() {
                    inner_type = quote! { #inner_type #token };
                }
                return quote! {
                    #ret::<#inner_type>
                };
            }
        }
        quote! {
            #ret::<#ty>
        }
    } else {
        ToTokens::into_token_stream(field_type)
    }
}

fn visit_attribute<V: ProtoTKVisitor>(
    _ctor: &syn::Ident,
    field: &syn::Field,
    field_ident: &TokenStream,
    attr: &syn::Attribute,
    v: &mut V,
) -> Option<TokenStream> {
    let (field_number, field_type) = match parse_attribute(attr) {
        Some(x) => x,
        None => {
            return None;
        }
    };
    let field_type = &field_type_tokens(field, &field_type);
    let ctor = quote! { ctor };
    Some(v.field_snippet(&ctor, field, field_ident, &field_number, field_type))
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
        for field in fields.named.iter() {
            let field_ident = &ToTokens::into_token_stream(&field.ident);
            for attr in field.attrs.iter() {
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
        for (index, field) in fields.unnamed.iter().enumerate() {
            let field_ident =
                syn::LitInt::new(&format!("{}", index), proc_macro2::Span::call_site());
            let field_ident = &ToTokens::into_token_stream(&field_ident);
            for attr in field.attrs.iter() {
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
        self.enum_snippet(ty_name, variants)
    }

    fn visit_enum_variant_named_fields(
        &mut self,
        _ty_name: &syn::Ident,
        _de: &syn::DataEnum,
        variant: &syn::Variant,
        fields: &syn::FieldsNamed,
    ) -> Self::VariantOutput {
        let (field_number, field_type) = parse_attributes(&variant.attrs);
        let variant_ident = &variant.ident;
        let ctor = quote! { Self :: #variant_ident };
        self.named_variant_snippet(&ctor, variant, &field_number, &field_type, fields)
    }

    fn visit_enum_variant_unnamed_fields(
        &mut self,
        _ty_name: &syn::Ident,
        _de: &syn::DataEnum,
        variant: &syn::Variant,
        fields: &syn::FieldsUnnamed,
    ) -> Self::VariantOutput {
        if fields.unnamed.len() != 1 {
            panic!("{}", USAGE);
        }
        let field = &fields.unnamed[0];
        let (field_number, field_type) = parse_attributes(&variant.attrs);
        let field_type = &field_type_tokens(field, &field_type);
        let variant_ident = &variant.ident;
        let ctor = quote! { Self :: #variant_ident };
        self.unnamed_variant_snippet(&ctor, variant, &field_number, field_type)
    }

    fn visit_enum_variant_unit(
        &mut self,
        _ty_name: &syn::Ident,
        _de: &syn::DataEnum,
        variant: &syn::Variant,
    ) -> Self::VariantOutput {
        let (field_number, _) = parse_attributes(&variant.attrs);
        let variant_ident = &variant.ident;
        let ctor = quote! { Self :: #variant_ident };
        self.unit_variant_snippet(&ctor, variant, &field_number)
    }
}

//////////////////////////////////////// PackMessageVisitor ////////////////////////////////////////

#[derive(Default)]
struct PackMessageVisitor {
    call: TokenStream,
}

impl PackMessageVisitor {
    fn new(call: TokenStream) -> Self {
        Self { call }
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
            let prototk_pa = prototk_pa.pack(::prototk::field_types::#field_type::field_packer(::prototk::FieldNumber::must(#field_number), &self.#field_ident));
        }
    }

    fn struct_snippet(&mut self, _ty_name: &syn::Ident, fields: &[TokenStream]) -> TokenStream {
        let call = &self.call;
        quote! {
            let prototk_pa = ::buffertk::stack_pack(());
            #(#fields;)*
            prototk_pa.#call
        }
    }

    fn named_variant_snippet(
        &mut self,
        ctor: &TokenStream,
        _variant: &syn::Variant,
        field_number: &syn::LitInt,
        _field_type: &syn::Path,
        fields: &syn::FieldsNamed,
    ) -> TokenStream {
        let mut enum_names = Vec::new();
        let mut field_decls = Vec::new();
        for field in fields.named.iter() {
            let enum_name = field.ident.clone();
            enum_names.push(field.ident.clone());
            let (enum_number, enum_type) = parse_attributes(&field.attrs);
            let enum_type = &field_type_tokens(field, &enum_type);
            let decl = quote! {
                let prototk_pa = prototk_pa.pack(::prototk::field_types::#enum_type::field_packer(::prototk::FieldNumber::must(#enum_number), #enum_name));
            };
            field_decls.push(decl);
        }
        let call = &self.call;
        quote! {
            #ctor { #(#enum_names,)* } => {
                let prototk_pa = ::buffertk::stack_pack(());
                #(#field_decls;)*
                ::buffertk::stack_pack(::prototk::Tag {
                    field_number: ::prototk::FieldNumber::must(#field_number),
                    wire_type: ::prototk::WireType::LengthDelimited,
                }).pack(prototk_pa.length_prefixed()).#call
            },
        }
    }

    fn unnamed_variant_snippet(
        &mut self,
        ctor: &TokenStream,
        variant: &syn::Variant,
        field_number: &syn::LitInt,
        field_type: &TokenStream,
    ) -> TokenStream {
        let call = &self.call;
        quote_spanned! { variant.span() =>
            #ctor(v) => {
                ::buffertk::stack_pack(::prototk::field_types::#field_type::field_packer(::prototk::FieldNumber::must(#field_number), v)).#call
            },
        }
    }

    fn unit_variant_snippet(
        &mut self,
        ctor: &TokenStream,
        variant: &syn::Variant,
        field_number: &syn::LitInt,
    ) -> TokenStream {
        let call = &self.call;
        quote_spanned! { variant.span() =>
            #ctor => {
                let prototk_empty: &[u8] = &[];
                ::buffertk::stack_pack(::prototk::field_types::bytes::field_packer(::prototk::FieldNumber::must(#field_number), &prototk_empty)).#call
            },
        }
    }

    fn enum_snippet(&mut self, _ty_name: &syn::Ident, variants: &[TokenStream]) -> TokenStream {
        quote! {
            match self {
                #(#variants)*
            }
        }
    }
}

/////////////////////////////////////// UnpackMessageVisitor ///////////////////////////////////////

#[derive(Default)]
struct UnpackMessageVisitor {}

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
            (#field_number, ::prototk::field_types::#field_type::WIRE_TYPE) => {
                let (prototk_tmp, _): (::prototk::field_types::#field_type, _) = Unpackable::unpack(field_value)?;
                ret.#field_ident.merge_field(prototk_tmp);
            },
        }
    }

    fn struct_snippet(&mut self, _ty_name: &syn::Ident, fields: &[TokenStream]) -> TokenStream {
        quote! {
            let mut ret = Self::default();
            let mut error: Option<::prototk::Error> = None;
            let fields = ::prototk::FieldIterator::new(buf, &mut error);
            for (tag, field_value) in fields {
                let num: u32 = tag.field_number.into();
                match (num, tag.wire_type) {
                    #(#fields)*
                    (_, _) => {},
                }
            }
            Ok((ret, &[]))
        }
    }

    fn named_variant_snippet(
        &mut self,
        ctor: &TokenStream,
        _variant: &syn::Variant,
        field_number: &syn::LitInt,
        _field_type: &syn::Path,
        fields: &syn::FieldsNamed,
    ) -> TokenStream {
        let mut enum_names = Vec::new();
        let mut field_decls = Vec::new();
        let mut field_blocks = Vec::new();
        let mut hydration = Vec::new();
        for field in fields.named.iter() {
            let enum_name = field.ident.clone();
            enum_names.push(field.ident.clone());
            let field_name = syn::Ident::new(
                &format!("prototk_field_{}", field.ident.to_token_stream()),
                field.span(),
            );
            let (enum_number, enum_type) = parse_attributes(&field.attrs);
            let enum_type = &field_type_tokens(field, &enum_type);
            let mut decl = quote! {};
            let default_type = &field.ty;
            let mut default_type_string = format!("{}", default_type.to_token_stream());
            default_type_string.retain(|c| !c.is_whitespace());
            if default_type_string == "[u8;64]" {
                decl = quote! {
                    #decl
                    let mut #field_name = [0u8; 64];
                }
            } else {
                decl = quote! {
                    #decl
                    let mut #field_name = <#default_type as Default>::default();
                };
            }
            field_decls.push(decl);
            let block = quote! {
                (#enum_number, ::prototk::field_types::#enum_type::WIRE_TYPE) => {
                    let enum_value: ::prototk::field_types::#enum_type = Unpackable::unpack(buf)?.0;
                    #field_name.merge_field(enum_value);
                },
            };
            field_blocks.push(block);
            let hydrate = quote! {
                #enum_name: #field_name,
            };
            hydration.push(hydrate);
        }
        quote! {
            (#field_number, ::prototk::WireType::LengthDelimited) => {
                let length: v64 = up.unpack()?;
                let mut error: Option<::prototk::Error> = None;
                let local_buf: &'b [u8] = &up.remain()[0..length.into()];
                up.advance(length.into());
                let fields = ::prototk::FieldIterator::new(local_buf, &mut error);
                #(#field_decls;)*
                for (tag, buf) in fields {
                    let num: u32 = tag.field_number.into();
                    match (num, tag.wire_type) {
                        #(#field_blocks)*
                        (_, _) => {
                            return Err(::prototk::Error::UnknownDiscriminant { discriminant: num }.into());
                        },
                    }
                }
                let ret = #ctor {
                    #(#hydration)*
                };
                Ok((ret, up.remain()))
            },
        }
    }

    fn unnamed_variant_snippet(
        &mut self,
        ctor: &TokenStream,
        variant: &syn::Variant,
        field_number: &syn::LitInt,
        field_type: &TokenStream,
    ) -> TokenStream {
        quote_spanned! { variant.span() =>
            (#field_number, ::prototk::field_types::#field_type::WIRE_TYPE) => {
                let tmp: ::prototk::field_types::#field_type = up.unpack()?;
                #[allow(clippy::useless_conversion)]
                Ok((#ctor(tmp.into_native().into()), up.remain()))
            },
        }
    }

    fn unit_variant_snippet(
        &mut self,
        ctor: &TokenStream,
        variant: &syn::Variant,
        field_number: &syn::LitInt,
    ) -> TokenStream {
        quote_spanned! { variant.span() =>
            (#field_number, ::prototk::WireType::LengthDelimited) => {
                let x: v64 = up.unpack()?;
                up.advance(x.into());
                Ok((#ctor, up.remain()))
            },
        }
    }

    fn enum_snippet(&mut self, _ty_name: &syn::Ident, variants: &[TokenStream]) -> TokenStream {
        quote! {
            let mut up = ::buffertk::Unpacker::new(buf);
            let tag: ::prototk::Tag = up.unpack()?;
            let num: u32 = tag.field_number.into();
            let wire_type: ::prototk::WireType = tag.wire_type;
            match (num, wire_type) {
                #(#variants)*
                _ => {
                    return Err(::prototk::Error::UnknownDiscriminant { discriminant: num }.into());
                },
            }
        }
    }
}
