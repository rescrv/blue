#![recursion_limit = "128"]
#![doc = include_str!("../README.md")]

extern crate proc_macro;
extern crate quote;
extern crate syn;

/////////////////////////////////////////// StructVisitor //////////////////////////////////////////

/// [StructVisitor] provides default implementations for panicking when visiting a struct.
/// Override [StructVisitor::visit_struct_named_fields],
/// [StructVisitor::visit_struct_unnamed_fields], and [StructVisitor::visit_struct_unit] to support
/// the three different struct types.
pub trait StructVisitor: Sized {
    /// Type of output this StructVisitor returns.
    type Output;

    /// Visit the struct and switch over the struct type.  This will call one of the other visit
    /// methods.
    fn visit_struct(&mut self, ty_name: &syn::Ident, ds: &syn::DataStruct) -> Self::Output {
        match ds.fields {
            syn::Fields::Named(ref fields) => self.visit_struct_named_fields(ty_name, ds, fields),
            syn::Fields::Unnamed(ref fields) => {
                self.visit_struct_unnamed_fields(ty_name, ds, fields)
            }
            syn::Fields::Unit => self.visit_struct_unit(ty_name, ds),
        }
    }

    /// Visit a struct with named fields.
    fn visit_struct_named_fields(
        &mut self,
        ty_name: &syn::Ident,
        ds: &syn::DataStruct,
        fields: &syn::FieldsNamed,
    ) -> Self::Output {
        _ = ty_name;
        _ = ds;
        _ = fields;
        panic!("{}", "structs with named fields are not supported");
    }

    /// Visit a struct with unnamed fields.
    fn visit_struct_unnamed_fields(
        &mut self,
        ty_name: &syn::Ident,
        ds: &syn::DataStruct,
        fields: &syn::FieldsUnnamed,
    ) -> Self::Output {
        _ = ty_name;
        _ = ds;
        _ = fields;
        panic!("{}", "structs with unnamed fields are not supported");
    }

    /// Visit a unit struct.
    fn visit_struct_unit(&mut self, ty_name: &syn::Ident, ds: &syn::DataStruct) -> Self::Output {
        _ = ty_name;
        _ = ds;
        panic!("{}", "unit structs are not supported");
    }
}

//////////////////////////////////////////// EnumVisitor ///////////////////////////////////////////

/// [EnumVisitor] provides default implementations for panicking when visiting an enum.  Provide
/// implementations of [EnumVisitor::combine_variants], and at least one of
/// [EnumVisitor::visit_enum_variant_named_field], [EnumVisitor::visit_enum_variant_unnamed_field],
/// and [EnumVisitor::visit_enum_variant_unit].
pub trait EnumVisitor: Sized {
    /// Type of output this EnumVisitor creates for an enum.
    type Output;
    /// Type of output this EnumVisitor creates for each variant.
    type VariantOutput;

    /// Visit all variants and combine them into one output.
    fn visit_enum(&mut self, ty_name: &syn::Ident, de: &syn::DataEnum) -> Self::Output {
        let mut variants = Vec::new();
        for v in de.variants.iter() {
            variants.push(self.visit_enum_variant(ty_name, de, v));
        }
        self.combine_variants(ty_name, de, &variants)
    }

    /// Combine the provided variants into one output.
    fn combine_variants(
        &mut self,
        ty_name: &syn::Ident,
        de: &syn::DataEnum,
        variants: &[Self::VariantOutput],
    ) -> Self::Output;

    /// Visit an enum, switching over its variant type.
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

    /// Visit an enum with [syn::FieldsNamed].
    fn visit_enum_variant_named_field(
        &mut self,
        _ty_name: &syn::Ident,
        _de: &syn::DataEnum,
        _variant: &syn::Variant,
        _fields: &syn::FieldsNamed,
    ) -> Self::VariantOutput {
        panic!("{}", "enum variants with named fields are not supported");
    }

    /// Visit an enum with [syn::FieldsUnnamed].
    fn visit_enum_variant_unnamed_field(
        &mut self,
        _ty_name: &syn::Ident,
        _de: &syn::DataEnum,
        _variant: &syn::Variant,
        _fields: &syn::FieldsUnnamed,
    ) -> Self::VariantOutput {
        panic!("{}", "enum variants with unnamed fields are not supported");
    }

    /// Visit a unit enum.
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
            let mut output = format!("struct {} {{\n", ty_name);
            for field in fields.named.iter() {
                output += &format!(
                    "    {}: {},\n",
                    field.ident.as_ref().unwrap(),
                    field.ty.clone().into_token_stream(),
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
            let mut output = format!("struct {}(", ty_name);
            let mut first = true;
            for field in fields.unnamed.iter() {
                if first {
                    output += &format!("{}", field.ty.clone().into_token_stream());
                } else {
                    output += &format!(", {}", field.ty.clone().into_token_stream());
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
            format!("struct {};", ty_name)
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
            let mut output = format!("enum {} {{\n", ty_name);
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
            let mut output = format!("    {} {{", variant.ident);
            let mut first = true;
            for field in fields.named.iter() {
                if first {
                    output += &format!(
                        " {}: {}",
                        field.ident.as_ref().unwrap(),
                        field.ty.clone().into_token_stream(),
                    );
                } else {
                    output += &format!(
                        ", {}: {}",
                        field.ident.as_ref().unwrap(),
                        field.ty.clone().into_token_stream(),
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
            let mut output = format!("    {}(", variant.ident);
            let mut first = true;
            for field in fields.unnamed.iter() {
                if first {
                    output += &format!("{}", field.ty.clone().into_token_stream());
                } else {
                    output += &format!(", {}", field.ty.clone().into_token_stream());
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
            format!("    {},\n", variant.ident)
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
