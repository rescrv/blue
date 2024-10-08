#![doc = include_str!("../README.md")]
#![recursion_limit = "128"]

extern crate proc_macro;
#[macro_use]
extern crate quote;
extern crate syn;

use proc_macro2::{Span, TokenStream};
use quote::ToTokens;
use syn::spanned::Spanned;
use syn::{parse_macro_input, DeriveInput};

use derive_util::EnumVisitor;

/////////////////////////////////////// #[derive(ZerrorCore)] //////////////////////////////////////

/// Derive ZerrorCore for an error.  This assumes a type has a core, and makes the with_* methods
/// for it.
#[proc_macro_derive(Z, attributes())]
pub fn derive_command_line(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    // `ty_name` holds the type's identifier.
    let ty_name = input.ident;

    let data = match input.data {
        syn::Data::Struct(_) => {
            panic!("structs are not supported");
        }
        syn::Data::Enum(de) => de,
        syn::Data::Union(_) => {
            panic!("unions are not supported");
        }
    };

    let mut cmv = CoreMethodsVisitor {};
    let core_methods = cmv.visit_enum(&ty_name, &data);
    let mut dmv = DisplayMethodVisitor {};
    let display_method = dmv.visit_enum(&ty_name, &data);
    let mut pemv = PartialEqMethodVisitor {};
    let partial_eq_method = pemv.visit_enum(&ty_name, &data);
    let gen = quote! {
        impl ::zerror::Z for #ty_name {
            type Error = Self;

            fn long_form(&self) -> String {
                format!("{}\n", self) + &self.core().long_form()
            }

            fn with_info<X: ::std::fmt::Debug>(mut self, name: &str, value: X) -> Self::Error {
                self.core_mut().set_info(name, value);
                self
            }

            fn with_lazy_info<F: FnOnce() -> String>(mut self, name: &str, value: F) -> Self::Error {
                self.core_mut().set_lazy_info(name, value);
                self
            }
        }

        impl ::std::fmt::Debug for #ty_name {
            fn fmt(&self, fmt: &mut ::std::fmt::Formatter<'_>) -> Result<(), ::std::fmt::Error> {
                <Self as ::std::fmt::Display>::fmt(self, fmt)
            }
        }

        #core_methods
        #display_method
        #partial_eq_method
    };
    gen.into()
}

//////////////////////////////////////// CommandLineVisitor ////////////////////////////////////////

struct CoreMethodsVisitor {}

impl EnumVisitor for CoreMethodsVisitor {
    type Output = TokenStream;
    type VariantOutput = TokenStream;

    /// Combine the provided variants into one output.
    fn combine_variants(
        &mut self,
        ty_name: &syn::Ident,
        _de: &syn::DataEnum,
        variants: &[Self::VariantOutput],
    ) -> Self::Output {
        let mut variant_sum = quote! {};
        for v in variants {
            let one = quote! {
                #variant_sum
                #v
            };
            variant_sum = one;
        }
        quote! {
            impl #ty_name {
                /// Get an immutable reference to this core.
                pub fn core(&self) -> &::zerror_core::ErrorCore {
                    match self {
                        #variant_sum
                    }
                }

                /// Get a mutable reference to this core.
                pub fn core_mut(&mut self) -> &mut ::zerror_core::ErrorCore {
                    match self {
                        #variant_sum
                    }
                }
            }
        }
    }

    /// Visit an enum with [syn::FieldsNamed].
    fn visit_enum_variant_named_field(
        &mut self,
        ty_name: &syn::Ident,
        _de: &syn::DataEnum,
        variant: &syn::Variant,
        _fields: &syn::FieldsNamed,
    ) -> Self::VariantOutput {
        let variant = &variant.ident;
        quote! {
            #ty_name::#variant { core, .. } => core,
        }
    }
}

/////////////////////////////////////// DisplayMethodVisitor ///////////////////////////////////////

struct DisplayMethodVisitor {}

impl EnumVisitor for DisplayMethodVisitor {
    type Output = TokenStream;
    type VariantOutput = TokenStream;

    /// Combine the provided variants into one output.
    fn combine_variants(
        &mut self,
        ty_name: &syn::Ident,
        _de: &syn::DataEnum,
        variants: &[Self::VariantOutput],
    ) -> Self::Output {
        let mut variant_sum = quote! {};
        for v in variants {
            variant_sum = quote! {
                #variant_sum
                #v
            };
        }
        quote! {
            impl ::std::fmt::Display for #ty_name {
                fn fmt(&self, fmt: &mut ::std::fmt::Formatter) -> Result<(), ::std::fmt::Error> {
                    match self {
                        #variant_sum
                    }
                }
            }
        }
    }

    /// Visit an enum with [syn::FieldsNamed].
    fn visit_enum_variant_named_field(
        &mut self,
        ty_name: &syn::Ident,
        _de: &syn::DataEnum,
        variant: &syn::Variant,
        fields: &syn::FieldsNamed,
    ) -> Self::VariantOutput {
        let mut fields_list_quote = quote! {};
        let mut fields_fmt_quote = quote! {};
        let mut first_field = true;
        for field in fields.named.iter() {
            if field.ident == Some(syn::Ident::new("core", field.span())) {
                continue;
            }
            let field_ident = &field.ident;
            if first_field {
                fields_list_quote = quote! {
                    #field_ident
                };
            } else {
                fields_list_quote = quote! {
                    #fields_list_quote, #field_ident
                };
            }
            let field_str = field_ident.clone().into_token_stream().to_string();
            fields_fmt_quote = quote! {
                #fields_fmt_quote
                .field(#field_str, #field_ident)
            };
            first_field = false;
        }
        let variant = &variant.ident;
        let variant_str = variant.clone().into_token_stream().to_string();
        quote! {
            #ty_name::#variant { core: _, #fields_list_quote } => {
                fmt.debug_struct(#variant_str)
                #fields_fmt_quote
                .finish()
            }
        }
    }
}

////////////////////////////////////// PartialEqMethodVisitor //////////////////////////////////////

struct PartialEqMethodVisitor {}

impl EnumVisitor for PartialEqMethodVisitor {
    type Output = TokenStream;
    type VariantOutput = TokenStream;

    /// Combine the provided variants into one output.
    fn combine_variants(
        &mut self,
        ty_name: &syn::Ident,
        _de: &syn::DataEnum,
        variants: &[Self::VariantOutput],
    ) -> Self::Output {
        let mut variant_sum = quote! {};
        for v in variants {
            variant_sum = quote! {
                #variant_sum
                #v
            };
        }
        quote! {
            impl Eq for #ty_name {}

            impl PartialEq for #ty_name {
                fn eq(&self, other: &#ty_name) -> bool {
                    match (self, other) {
                        #variant_sum
                        (_, _) => { false },
                    }
                }
            }
        }
    }

    /// Visit an enum with [syn::FieldsNamed].
    fn visit_enum_variant_named_field(
        &mut self,
        ty_name: &syn::Ident,
        _de: &syn::DataEnum,
        variant: &syn::Variant,
        fields: &syn::FieldsNamed,
    ) -> Self::VariantOutput {
        let mut fields_list_lhs = quote! {};
        let mut fields_list_rhs = quote! {};
        let mut fields_compare = quote! {};
        let mut num_fields = 0;
        for field in fields.named.iter() {
            if field.ident == Some(syn::Ident::new("core", field.span())) {
                continue;
            }
            let field_ident = &field.ident;
            let field_lhs =
                syn::Ident::new(&format!("zerror_{}_lhs", num_fields), Span::call_site());
            let field_rhs =
                syn::Ident::new(&format!("zerror_{}_rhs", num_fields), Span::call_site());
            if num_fields == 0 {
                fields_list_lhs = quote! {
                    #field_ident: #field_lhs
                };
                fields_list_rhs = quote! {
                    #field_ident: #field_rhs
                };
                fields_compare = quote! {
                    #field_lhs == #field_rhs
                }
            } else {
                fields_list_lhs = quote! {
                    #fields_list_lhs, #field_ident: #field_lhs
                };
                fields_list_rhs = quote! {
                    #fields_list_rhs, #field_ident: #field_rhs
                };
                fields_compare = quote! {
                    #fields_compare && #field_lhs == #field_rhs
                }
            }
            num_fields += 1;
        }
        if num_fields == 0 {
            fields_compare = quote! { true }
        }
        let variant_ident = &variant.ident;
        quote! {
            (#ty_name::#variant_ident { core: _, #fields_list_lhs },
             #ty_name::#variant_ident { core: _, #fields_list_rhs }) => {
                #fields_compare
            },
        }
    }
}
