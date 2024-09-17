////////////////////////////////////////////// zerror //////////////////////////////////////////////

#[macro_export]
macro_rules! zerror {
    (@inner
     $(#[$doc:meta])*
     $error_name:ident
     [$($(#[$variant1_doc:meta])* $variant1_type:ident as $variant1_ctor:ident counter $counter1_name:path
         { $($(#[$field1_doc:meta])* $field1_name:ident : $field1_type:ty),* };)*]
     [$($(#[$variant2_doc:meta])* $variant2_type:ident from $(#[$from2_doc:meta])* $from2_type:ty as $variant2_ctor:ident counter $counter2_name:path;)*]
    ) => {
        $(#[$doc])*
        pub enum $error_name {
            $($(#[$variant1_doc])* $variant1_type {
                #[prototk(1, message)]
                core: zerror_core::ErrorCore,
                $($(#[$field1_doc])* $field1_name : $field1_type,)*
            },)*
            $($(#[$variant2_doc])* $variant2_type {
                #[prototk(1, message)]
                core: zerror_core::ErrorCore,
                $(#[$from2_doc])* what : $from2_type,
            },)*
        }

        impl $error_name {
            /// Get an immutable reference to this core.
            pub fn core(&self) -> &::zerror_core::ErrorCore {
                match self {
                    $($error_name ::$variant1_type { core, $($field1_name: _,)* } => { core },)*
                    $($error_name ::$variant2_type { core, what: _ } => { core },)*
                }
            }

            /// Get a mutable reference to this core.
            pub fn core_mut(&mut self) -> &mut ::zerror_core::ErrorCore {
                match self {
                    $($error_name ::$variant1_type { core, $($field1_name: _,)* } => { core },)*
                    $($error_name ::$variant2_type { core, what: _ } => { core },)*
                }
            }

            $(
            pub fn $variant1_ctor($($field1_name: impl Into<$field1_type>),*) -> Self {
                Self::$variant1_type {
                    core: zerror_core::ErrorCore::new(&$counter1_name),
                    $($field1_name: $field1_name.into(),)*
                }
            }
            )*
        }

        impl ::zerror::Z for $error_name {
            type Error = Self;

            fn long_form(&self) -> String {
                format!("{}\n", self) + &self.core().long_form()
            }

            fn with_token(self, name: &str, value: &str) -> Self::Error {
                self
            }

            fn with_url(self, name: &str, value: &str) -> Self::Error {
                self
            }

            fn with_variable<X: ::std::fmt::Debug>(self, name: &str, value: X) -> Self::Error {
                self
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

        impl std::fmt::Debug for $error_name {
            fn fmt(&self, fmt: &mut ::std::fmt::Formatter) -> Result<(), ::std::fmt::Error> {
                match self {
                    _ => { Ok(()) },
                }
            }
        }

        impl std::fmt::Display for $error_name {
            fn fmt(&self, fmt: &mut ::std::fmt::Formatter) -> Result<(), ::std::fmt::Error> {
                use zerror::Z;
                write!(fmt, "{}", self.long_form())
            }
        }

        $(
        impl From<$from2_type> for $error_name {
            fn from(err: $from2_type) -> Self {
                Self::$variant2_type {
                    core: zerror_core::ErrorCore::new(&$counter2_name),
                    what: err,
                }
            }
        }
        )*
    };
    (@inner
     $(#[$doc:meta])*
     $error_name:ident
     [$($(#[$variant1_doc:meta])* $variant1_type:ident as $variant1_ctor:ident counter $counter1_name:path
         { $($(#[$field1_doc:meta])* $field1_name:ident : $field1_type:ty),* };)*]
     [$($(#[$variant2_doc:meta])* $variant2_type:ident from $(#[$from2_doc:meta])* $from2_type:ty as $variant2_ctor:ident counter $counter2_name:path;)*]
     $(#[$variant_doc:meta])* $variant_type:ident from $(#[$from_doc:meta])* $from_type:ty as $variant_ctor:ident counter $counter_name:path,
     $($tt:tt)*
    ) => {
        zerror!(@inner
            $(#[$doc])*
            $error_name
            [$($(#[$variant1_doc])* $variant1_type as $variant1_ctor counter $counter1_name
                { $($(#[$field1_doc])* $field1_name : $field1_type),* };)*]
            [$($(#[$variant2_doc])* $variant2_type from $(#[$from2_doc])* $from2_type as $variant2_ctor counter $counter2_name;)*
                $(#[$variant_doc])* $variant_type from $(#[$from_doc])* $from_type as $variant_ctor counter $counter_name;]
            $($tt)*);
    };
    (@inner
     $(#[$doc:meta])*
     $error_name:ident
     [$($(#[$variant1_doc:meta])* $variant1_type:ident as $variant1_ctor:ident counter $counter1_name:path
         { $($(#[$field1_doc:meta])* $field1_name:ident : $field1_type:ty),* };)*]
     [$($(#[$variant2_doc:meta])* $variant2_type:ident from $(#[$from2_doc:meta])* $from2_type:ty as $variant2_ctor:ident counter $counter2_name:path;)*]
     $(#[$variant_doc:meta])* $variant_type:ident from $(#[$from_doc:meta])* $from_type:ty as $variant_ctor:ident,
     $($tt:tt)*
    ) => {
        zerror!(@inner
            $(#[$doc])*
            $error_name
            [$($(#[$variant1_doc])* $variant1_type as $variant1_ctor counter $counter1_name
                { $($(#[$field1_doc])* $field1_name : $field1_type),* };)*]
            [$($(#[$variant2_doc])* $variant2_type from $(#[$from2_doc])* $from2_type as $variant2_ctor counter $counter2_name;)*
                $(#[$variant_doc])* $variant_type from $(#[$from_doc])* $from_type as $variant_ctor counter zerror_core::DEFAULT_ERROR_CORE;]
            $($tt)*);
    };
    (@inner
     $(#[$doc:meta])*
     $error_name:ident
     [$($(#[$variant1_doc:meta])* $variant1_type:ident as $variant1_ctor:ident counter $counter1_name:path
         { $($(#[$field1_doc:meta])* $field1_name:ident : $field1_type:ty),* };)*]
     [$($(#[$variant2_doc:meta])* $variant2_type:ident from $(#[$from2_doc:meta])* $from2_type:ty
         as $variant2_ctor:ident counter $counter2_name:path;)*]
     $(#[$variant_doc:meta])* $variant_type:ident as $variant_ctor:ident counter $counter_name:path
         { $($(#[$field_doc:meta])* $field_name:ident : $field_type:ty,)* },
     $($tt:tt)*
    ) => {
        zerror!(@inner
            $(#[$doc])*
            $error_name
            [$($(#[$variant1_doc])* $variant1_type as $variant1_ctor counter $counter1_name
                { $($(#[$field1_doc])* $field1_name : $field1_type),* };)*
                $(#[$variant_doc])* $variant_type as $variant_ctor counter $counter_name
                { $($(#[$field_doc])* $field_name : $field_type),* };]
            [$($(#[$variant2_doc])* $variant2_type from $(#[$from2_doc])* $from2_type as $variant2_ctor counter $counter2_name;)*]
            $($tt)*);
    };
    (@inner
     $(#[$doc:meta])*
     $error_name:ident
     [$($(#[$variant1_doc:meta])* $variant1_type:ident as $variant1_ctor:ident counter $counter1_name:path
         { $($(#[$field1_doc:meta])* $field1_name:ident : $field1_type:ty),* };)*]
     [$($(#[$variant2_doc:meta])* $variant2_type:ident from $(#[$from2_doc:meta])* $from2_type:ty
         as $variant2_ctor:ident counter $counter2_name:path;)*]
     $(#[$variant_doc:meta])* $variant_type:ident as $variant_ctor:ident
         { $($(#[$field_doc:meta])* $field_name:ident : $field_type:ty,)* },
     $($tt:tt)*
    ) => {
        zerror!(@inner
            $(#[$doc])*
            $error_name
            [$($(#[$variant1_doc])* $variant1_type as $variant1_ctor counter $counter1_name
                { $($(#[$field1_doc])* $field1_name : $field1_type),* };)*
                $(#[$variant_doc])* $variant_type as $variant_ctor counter zerror_core::DEFAULT_ERROR_CORE
                { $($(#[$field_doc])* $field_name : $field_type),* };]
            [$($(#[$variant2_doc])* $variant2_type from $(#[$from2_doc])* $from2_type as $variant2_ctor counter $counter2_name;)*]
            $($tt)*);
    };
    ($(#[$doc:meta])* $error_name:ident { $($tt:tt)* }) => {
        zerror!(@inner $(#[$doc])* $error_name [] [] $($tt)*);
    };
}
