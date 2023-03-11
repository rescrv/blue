#![recursion_limit = "128"]

extern crate proc_macro;
extern crate quote;
extern crate syn;

/////////////////////////////////////////// StructVisitor //////////////////////////////////////////

pub trait StructVisitor: Sized {
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
                self.visit_enum_variant_named_field(ty_name, de, variant, fields)
            }
            syn::Fields::Unnamed(ref fields) => {
                self.visit_enum_variant_unnamed_field(ty_name, de, variant, fields)
            }
            syn::Fields::Unit => self.visit_enum_variant_unit(ty_name, de, variant),
        }
    }

    fn visit_enum_variant_named_field(
        &mut self,
        _ty_name: &syn::Ident,
        _de: &syn::DataEnum,
        _variant: &syn::Variant,
        _fields: &syn::FieldsNamed,
    ) -> Self::VariantOutput {
        panic!("{}", "enum variants with named fields are not supported");
    }

    fn visit_enum_variant_unnamed_field(
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

/////////////////////////////////////////////// tests //////////////////////////////////////////////

#[cfg(test)]
mod structs {
    use std::str::FromStr;

    use proc_macro2::TokenStream;
    use quote::ToTokens;
    use syn::DeriveInput;

    use super::*;

    struct TestStructVisitor {}

    impl StructVisitor for TestStructVisitor {
        type Output = String;

        fn visit_struct_named_fields(
            &mut self,
            ty_name: &syn::Ident,
            _ds: &syn::DataStruct,
            fields: &syn::FieldsNamed,
        ) -> String {
            let mut output = format!("struct {} {{\n", ty_name.to_string());
            for field in fields.named.iter() {
                output += &format!(
                    "    {}: {},\n",
                    field.ident.as_ref().unwrap(),
                    field.ty.clone().into_token_stream().to_string()
                );
            }
            output += "}";
            output
        }

        fn visit_struct_unnamed_fields(
            &mut self,
            ty_name: &syn::Ident,
            _ds: &syn::DataStruct,
            fields: &syn::FieldsUnnamed,
        ) -> Self::Output {
            let mut output = format!("struct {}(", ty_name.to_string());
            let mut first = true;
            for field in fields.unnamed.iter() {
                if first {
                    output += &format!("{}", field.ty.clone().into_token_stream().to_string());
                } else {
                    output += &format!(", {}", field.ty.clone().into_token_stream().to_string());
                }
                first = false;
            }
            output += ");";
            output
        }

        fn visit_struct_unit(
            &mut self,
            ty_name: &syn::Ident,
            _ds: &syn::DataStruct,
        ) -> Self::Output {
            format!("struct {};", ty_name.to_string())
        }
    }

    fn test_struct(expect: &str) {
        let token_stream = TokenStream::from_str(expect).unwrap();
        let input: DeriveInput = syn::parse2(token_stream).unwrap();
        let mut visitor = TestStructVisitor {};
        let output = match input.data {
            syn::Data::Struct(ref ds) => visitor.visit_struct(&input.ident, ds),
            syn::Data::Enum(_) => {
                panic!("did not expect an enum");
            }
            syn::Data::Union(_) => {
                panic!("did not expect a union");
            }
        };
        assert_eq!(expect, output);
    }

    #[test]
    fn named_fields() {
        test_struct("struct NamedFields {\n    x: u16,\n    y: u32,\n    z: u64,\n}");
    }

    #[test]
    fn unnamed_fields() {
        test_struct("struct UnnamedFields(u16, u32, u64);");
    }

    #[test]
    fn unit() {
        test_struct("struct Unit;");
    }
}

#[cfg(test)]
mod enums {
    use std::str::FromStr;

    use proc_macro2::TokenStream;
    use quote::ToTokens;
    use syn::DeriveInput;

    use super::*;

    struct TestEnumVisitor {}

    impl EnumVisitor for TestEnumVisitor {
        type Output = String;
        type VariantOutput = String;

        fn combine_variants(
            &mut self,
            ty_name: &syn::Ident,
            _de: &syn::DataEnum,
            variants: &[Self::VariantOutput],
        ) -> Self::Output {
            let mut output = format!("enum {} {{\n", ty_name.to_string());
            for variant in variants {
                output += variant;
            }
            output += "}";
            output
        }

        fn visit_enum_variant_named_field(
            &mut self,
            _ty_name: &syn::Ident,
            _de: &syn::DataEnum,
            variant: &syn::Variant,
            fields: &syn::FieldsNamed,
        ) -> Self::VariantOutput {
            let mut output = format!("    {} {{", variant.ident.to_string());
            let mut first = true;
            for field in fields.named.iter() {
                if first {
                    output += &format!(
                        " {}: {}",
                        field.ident.as_ref().unwrap().to_string(),
                        field.ty.clone().into_token_stream().to_string()
                    );
                } else {
                    output += &format!(
                        ", {}: {}",
                        field.ident.as_ref().unwrap().to_string(),
                        field.ty.clone().into_token_stream().to_string()
                    );
                }
                first = false;
            }
            output += " },\n";
            output
        }

        fn visit_enum_variant_unnamed_field(
            &mut self,
            _ty_name: &syn::Ident,
            _de: &syn::DataEnum,
            variant: &syn::Variant,
            fields: &syn::FieldsUnnamed,
        ) -> Self::VariantOutput {
            let mut output = format!("    {}(", variant.ident.to_string());
            let mut first = true;
            for field in fields.unnamed.iter() {
                if first {
                    output += &format!("{}", field.ty.clone().into_token_stream().to_string());
                } else {
                    output += &format!(", {}", field.ty.clone().into_token_stream().to_string());
                }
                first = false;
            }
            output += "),\n";
            output
        }

        fn visit_enum_variant_unit(
            &mut self,
            _ty_name: &syn::Ident,
            _de: &syn::DataEnum,
            variant: &syn::Variant,
        ) -> Self::VariantOutput {
            format!("    {},\n", variant.ident.to_string())
        }
    }

    fn test_enum(expect: &str) {
        let token_stream = TokenStream::from_str(expect).unwrap();
        let input: DeriveInput = syn::parse2(token_stream).unwrap();
        let mut visitor = TestEnumVisitor {};
        let output = match input.data {
            syn::Data::Struct(_) => {
                panic!("did not expect a struct");
            }
            syn::Data::Enum(ref de) => visitor.visit_enum(&input.ident, de),
            syn::Data::Union(_) => {
                panic!("did not expect a union");
            }
        };
        assert_eq!(expect, output);
    }

    #[test]
    fn named_variant() {
        test_enum("enum NamedVariant {\n    Point { x: u64, y: u64 },\n}");
    }

    #[test]
    fn unnamed_variant() {
        test_enum("enum UnnamedVariant {\n    Point(u64, u64),\n}")
    }

    #[test]
    fn unit_variant() {
        test_enum("enum UnitVariant {\n    Something,\n}")
    }
}
