//! See arrrg for a description of this crate.

#![recursion_limit = "128"]

extern crate proc_macro;
#[macro_use]
extern crate quote;
extern crate syn;

use proc_macro2::TokenStream;
use quote::ToTokens;
use syn::{parse_macro_input, DeriveInput};

use derive_util::StructVisitor;

////////////////////////////////////// #[derive(CommandLine)] ///////////////////////////////////

#[proc_macro_derive(CommandLine, attributes(arrrg, help))]
pub fn derive_command_line(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
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

    let mut clv = CommandLineVisitor {};
    let (add_opts, matches, canonical_command_line) = clv.visit_struct(&ty_name, data);

    let gen = quote! {
        impl #impl_generics ::arrrg::CommandLine for #ty_name #ty_generics #where_clause {
            fn add_opts(&self, prefix: Option<&str>, opts: &mut getopts::Options) {
                #add_opts
            }

            fn matches(&mut self, prefix: Option<&str>, matches: &getopts::Matches) {
                #matches
            }

            fn canonical_command_line(&self, prefix: Option<&str>) -> Vec<String> {
                let dflt = Self::default();
                let mut result = Vec::new();
                #canonical_command_line
                result
            }
        }
    };
    gen.into()
}

//////////////////////////////////////// CommandLineVisitor ////////////////////////////////////////

fn type_is_option(ty: &syn::Type) -> bool {
    if let syn::Type::Path(ty) = ty {
        if ty.into_token_stream().to_string().starts_with("Option <") {
            return true;
        }
    }
    false
}

struct CommandLineVisitor {
}

impl StructVisitor for CommandLineVisitor {
    type Output = (TokenStream, TokenStream, TokenStream);

    fn visit_struct_named_fields(
        &mut self,
        _ty_name: &syn::Ident,
        _ds: &syn::DataStruct,
        fields: &syn::FieldsNamed,
    ) -> Self::Output {
        let mut add_opts = TokenStream::default();
        let mut matches = TokenStream::default();
        let mut canonical_command_line = TokenStream::default();
        'iterating_fields:
        for field in fields.named.iter() {
            if let Some(field_ident) = &field.ident {
                let field_arg = field_ident.to_string().replace('_', "-");
                let field_meta = match parse_meta(&field.attrs) {
                    Some(field_meta) => field_meta,
                    None => {
                        continue 'iterating_fields;
                    }
                };
                let field_type = field_meta.field_type;
                let help_string = field_meta.help_string;
                let hint_text = field_meta.hint_text.unwrap_or("".to_string());

                // Add options to the Options struct.
                let optopt = match field_type {
                    FieldType::Flag => {
                        quote! {
                            opts.optflag("", &arg_str, #help_string);
                        }
                    },
                    FieldType::Optional => {
                        quote! {
                            opts.optopt("", &arg_str, #help_string, #hint_text);
                        }
                    },
                    FieldType::Required => {
                        quote! {
                            opts.reqopt("", &arg_str, #help_string, #hint_text);
                        }
                    },
                    FieldType::Nested => {
                        quote! {
                            self.#field_ident.add_opts(Some(&arg_str), opts);
                        }
                    },
                };
                add_opts = quote !{
                    #add_opts
                    let arg_str = arrrg::getopt_str(prefix, #field_arg);
                    #optopt
                };

                // Retrieve values from the Matches struct.
                matches = quote! {
                    #matches
                    let arg_str = arrrg::getopt_str(prefix, #field_arg);
                };
                match field_type {
                    FieldType::Flag => {
                        matches = quote! {
                            #matches
                            if matches.opt_present(&arg_str) {
                                self.#field_ident = true;
                            }
                        };
                    },
                    FieldType::Optional => {
                        if type_is_option(&field.ty) {
                            matches = quote! {
                                #matches
                                match matches.opt_str(&arg_str) {
                                    Some(s) => {
                                        self.#field_ident = Some(arrrg::parse_field(&arg_str, &s));
                                    },
                                    None => {},
                                };
                            };
                        } else {
                            matches = quote! {
                                #matches
                                match matches.opt_str(&arg_str) {
                                    Some(s) => {
                                        self.#field_ident = arrrg::parse_field(&arg_str, &s);
                                    },
                                    None => {},
                                };
                            };
                        }
                    },
                    FieldType::Required => {
                        matches = quote! {
                            #matches
                            match matches.opt_str(&arg_str) {
                                Some(s) => {
                                    self.#field_ident = arrrg::parse_field(&arg_str, &s);
                                },
                                None => {
                                    panic!("required field --{} is missing", arg_str);
                                },
                            };
                        };
                    },
                    FieldType::Nested => {
                        matches = quote! {
                            #matches
                            self.#field_ident.matches(Some(&arg_str), matches);
                        };
                    },
                };

                // Construct canonical command lines.
                canonical_command_line = quote! {
                    #canonical_command_line
                    let arg_str = arrrg::getopt_str(prefix, #field_arg);
                    let flag_str = arrrg::dashed_str(prefix, #field_arg);
                };
                match field_type {
                    FieldType::Flag => {
                        canonical_command_line = quote! {
                            #canonical_command_line
                            if self.#field_ident {
                                result.push(flag_str);
                            }
                        };
                    },
                    FieldType::Optional => {
                        if type_is_option(&field.ty) {
                            canonical_command_line = quote! {
                                #canonical_command_line
                                if let Some(ref ident) = self.#field_ident {
                                    result.push(flag_str);
                                    result.push(ident.to_string());
                                }
                            };
                        } else {
                            canonical_command_line = quote! {
                                #canonical_command_line
                                if self.#field_ident != dflt.#field_ident {
                                    result.push(flag_str);
                                    result.push(self.#field_ident.to_string());
                                }
                            }
                        }
                    },
                    FieldType::Required => {
                        canonical_command_line = quote! {
                            #canonical_command_line
                            result.push(flag_str);
                            result.push(self.#field_ident.to_string());
                        };
                    },
                    FieldType::Nested => {
                        canonical_command_line = quote! {
                            #canonical_command_line
                            result.append(&mut self.#field_ident.canonical_command_line(Some(&arg_str)));
                        };
                    },
                };
            }
        }
        (add_opts, matches, canonical_command_line)
    }
}

//////////////////////////////////////////// attributes ////////////////////////////////////////////

const USAGE: &str = "must provide attributes of the form `arrrg(flag)`, `arrrg(opt)`, `arrrg(req)`, or `arrrg(nested)`";
const META_PATH: &str = "arrrg";

#[derive(Debug, Eq, PartialEq)]
enum FieldType {
    Flag,
    Optional,
    Required,
    Nested,
}

struct FlagMeta {
    field_type: FieldType,
    help_string: String,
    hint_text: Option<String>,
}

fn parse_meta_one(attr: &syn::Attribute) -> Option<FlagMeta> {
    let meta = &attr.parse_meta().unwrap();
    if meta.path().clone().into_token_stream().to_string() != META_PATH {
        return None;
    }
    let meta_list = match meta {
        syn::Meta::Path(_) => {
            panic!("meta path: {}", USAGE);
        }
        syn::Meta::List(ref ml) => ml,
        syn::Meta::NameValue(_) => {
            panic!("meta name value: {}", USAGE);
        }
    };
    if meta_list.nested.is_empty() || meta_list.nested.len() > 3 {
        panic!("meta list length: {}", USAGE);
    }
    let field_type = match &meta_list.nested[0] {
        syn::NestedMeta::Meta(field_type) => {
            field_type.into_token_stream().to_string()
        },
        syn::NestedMeta::Lit(_) => {
            panic!("{}", USAGE);
        },
    };
    let field_type: &str = &field_type;
    let field_type = match field_type {
        "flag" => FieldType::Flag,
        "opt" => FieldType::Optional,
        "req" => FieldType::Required,
        "nest" => FieldType::Nested,
        "optional" => FieldType::Optional,
        "required" => FieldType::Required,
        "nested" => FieldType::Nested,
        _ => {
            panic!("Unknown field_type {:?}", field_type)
        }
    };
    let help_string = if meta_list.nested.len() > 1 {
        meta_list.nested[1].clone().into_token_stream().to_string()
    } else {
        "default help text".to_string()
    };
    let hint_text = if meta_list.nested.len() > 2 {
        Some(meta_list.nested[2].clone().into_token_stream().to_string())
    } else {
        None
    };
    Some(FlagMeta {
        field_type,
        help_string,
        hint_text,
    })
}

fn parse_meta(attrs: &[syn::Attribute]) -> Option<FlagMeta> {
    for attr in attrs.iter() {
        if let Some(field_meta) = parse_meta_one(attr) {
            return Some(field_meta);
        }
    }
    None
}
