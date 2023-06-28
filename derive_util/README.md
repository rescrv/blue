derive_util
===========

derive_util is a library for writing struct and enum visitors.

struct visitor
--------------

The struct visitor provides a method that dispatches over the type of struct.  It is up to the implementor to implement
for named-fields, unnamed-fields, and the unit struct.

To override named fields, declare method [visit_struct_named_fields].

```
fn visit_struct_named_fields(
    &mut self,
    ty_name: &syn::Ident,
    ds: &syn::DataStruct,
    fields: &syn::FieldsNamed,
) -> Self::Output;
```

To override unnamed fields, declare method [visit_struct_unnamed_fields].

```
fn visit_struct_unnamed_fields(
    &mut self,
    ty_name: &syn::Ident,
    ds: &syn::DataStruct,
    fields: &syn::FieldsUnnamed,
) -> Self::Output;
```

To override the unit struct, declare method [visit_struct_unit].

```
fn visit_struct_unit(&mut self, _ty_name: &syn::Ident, _ds: &syn::DataStruct) -> Self::Output;
```

enum visitor
------------

To override the struct with named fields, declare method [visit_enum_variant_named_field].

```
fn visit_enum_variant_named_field(
    &mut self,
    ty_name: &syn::Ident,
    de: &syn::DataEnum,
    variant: &syn::Variant,
    fields: &syn::FieldsNamed,
) -> Self::VariantOutput;
```

To override the struct with unnamed fields, declare method [visit_enum_variant_unnamed_field].

```
fn visit_enum_variant_unnamed_field(
    &mut self,
    ty_name: &syn::Ident,
    de: &syn::DataEnum,
    variant: &syn::Variant,
    fields: &syn::FieldsUnnamed,
) -> Self::VariantOutput;
```

To override the unit enum, declare method [visit_enum_variant_unit].

```
fn visit_enum_variant_unit(
    &mut self,
    ty_name: &syn::Ident,
    de: &syn::DataEnum,
    variant: &syn::Variant,
) -> Self::VariantOutput;
```

Each variant returns `Self::VariantOutput`.  Combine these outputs into one `Self::Output`.

```
fn combine_variants(
    &mut self,
    ty_name: &syn::Ident,
    de: &syn::DataEnum,
    variants: &[Self::VariantOutput],
) -> Self::Output;
```
