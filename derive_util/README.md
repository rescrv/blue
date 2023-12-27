derive_util
===========

derive_util provides tools for derive macros.

struct visitor
--------------

The struct visitor provides a method that dispatches over the type of struct.  It is up to the implementor to implement
for named-fields, unnamed-fields, and the unit struct.

To override named fields, declare method [StructVisitor::visit_struct_named_fields].

```ignore
fn visit_struct_named_fields(
    &mut self,
    ty_name: &syn::Ident,
    ds: &syn::DataStruct,
    fields: &syn::FieldsNamed,
) -> Self::Output;
```

To override unnamed fields, declare method [StructVisitor::visit_struct_unnamed_fields].

```ignore
fn visit_struct_unnamed_fields(
    &mut self,
    ty_name: &syn::Ident,
    ds: &syn::DataStruct,
    fields: &syn::FieldsUnnamed,
) -> Self::Output;
```

To override the unit struct, declare method [StructVisitor::visit_struct_unit].

```ignore
fn visit_struct_unit(&mut self, _ty_name: &syn::Ident, _ds: &syn::DataStruct) -> Self::Output;
```

enum visitor
------------

To override the struct with named fields, declare method [EnumVisitor::visit_enum_variant_named_field].

```ignore
fn visit_enum_variant_named_field(
    &mut self,
    ty_name: &syn::Ident,
    de: &syn::DataEnum,
    variant: &syn::Variant,
    fields: &syn::FieldsNamed,
) -> Self::VariantOutput;
```

To override the struct with unnamed fields, declare method [EnumVisitor::visit_enum_variant_unnamed_field].

```ignore
fn visit_enum_variant_unnamed_field(
    &mut self,
    ty_name: &syn::Ident,
    de: &syn::DataEnum,
    variant: &syn::Variant,
    fields: &syn::FieldsUnnamed,
) -> Self::VariantOutput;
```

To override the unit enum, declare method [EnumVisitor::visit_enum_variant_unit].

```ignore
fn visit_enum_variant_unit(
    &mut self,
    ty_name: &syn::Ident,
    de: &syn::DataEnum,
    variant: &syn::Variant,
) -> Self::VariantOutput;
```

Each variant returns `Self::VariantOutput`.  Combine these outputs into one `Self::Output`.

```ignore
fn combine_variants(
    &mut self,
    ty_name: &syn::Ident,
    de: &syn::DataEnum,
    variants: &[Self::VariantOutput],
) -> Self::Output;
```

Status
------

Maintenance track.  The library is considered stable and will be put into maintenance mode if unchanged for one year.
This library was last updated 2023-09-23.

Scope
-----

This library will provide visitors for the core rust data types for use in derive macros.

Warts
-----

- The library is not complete enough to be used in [prototk](https://crates.io/crates/prototk) from which it was
  derived.

Documentation
-------------

The latest documentation is always available at [docs.rs](https://docs.rs/derive_util/latest/derive_util/).
