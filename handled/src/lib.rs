use std::fmt::Debug;

/////////////////////////////////////////////// Core ///////////////////////////////////////////////

#[derive(Clone, Debug, Default, Eq, PartialEq, Hash)]
pub struct Core {
    backtrace: String,
    info: Vec<(String, String)>,
}

impl Core {
    pub fn new() -> Self {
        let backtrace = std::backtrace::Backtrace::capture().to_string();
        Self {
            backtrace,
            info: vec![],
        }
    }

    pub fn with_info(&mut self, name: &str, value: impl Debug) {
        self.info.push((name.to_string(), format!("{value:#?}")));
    }

    pub fn without_backtrace(&mut self) {
        self.backtrace = String::new();
    }
}

////////////////////////////////////////////// HasCore /////////////////////////////////////////////

pub trait HasCore {
    fn core(&self) -> &Core;
    fn core_mut(&mut self) -> &mut Core;
}

///////////////////////////////////////////// ResultExt ////////////////////////////////////////////

pub trait ResultExt {
    fn with_info(self, name: &str, value: impl Debug) -> Self;
    fn with_lazy_info(self, name: &str, value: impl FnOnce() -> String) -> Self;
    fn without_backtrace(self) -> Self;
}

impl<T, E: HasCore> ResultExt for Result<T, E> {
    fn with_info(self, name: &str, value: impl Debug) -> Self {
        self.map_err(|e| {
            let mut e = e;
            e.core_mut().with_info(name, value);
            e
        })
    }

    fn with_lazy_info(self, name: &str, value: impl FnOnce() -> String) -> Self {
        self.map_err(|e| {
            let mut e = e;
            e.core_mut().with_info(name, value());
            e
        })
    }

    fn without_backtrace(self) -> Self {
        self.map_err(|mut e| {
            e.core_mut().without_backtrace();
            e
        })
    }
}

////////////////////////////////////////////// handled /////////////////////////////////////////////

#[macro_export]
macro_rules! handled {
    ($(#[$($type_attrs:meta)*])*
        $name:ident { $($tt:tt)* }) => {
        handled!(@inner $(#[$($type_attrs)*])* $name [] $($tt)*);
    };

    (@inner
        $(#[$($type_attrs:meta)*])*
        $name:ident
        [
            $(
                (
                    $(#[$($variant_attrs:meta)*])*
                    $variant:ident,
                    $function:ident,
                    $counter:path,
                    $(#[$($core_attrs:meta)*])*
                    (
                        $(
                            $(#[$($field_attrs:meta)*])*
                            $field:ident: $field_type:path,
                        )*
                    )
                )
            )*
        ]
    ) => {
        $(#[$($type_attrs)*])*
        pub enum $name {
            $(
                $(#[$($variant_attrs)*])*
                $variant {
                    $(#[$($core_attrs)*])*
                    core: Box<$crate::Core>,
                    $(
                        $(#[$($field_attrs)*])*
                        $field: $field_type,
                    )*
                },
            )*
        }

        impl $name {
            $(pub fn $function($($field: $field_type,)*) -> Self {
                $counter.click();
                $name::$variant {
                    core: Box::new($crate::Core::new()),
                    $($field,)*
                }
            })*
        }

        impl std::fmt::Debug for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                match self {
                    $(
                        $name::$variant { core, $($field),* } => {
                            let mut dbg = f.debug_struct(concat!(stringify!($name), "::", stringify!($variant)));
                            $(
                                dbg.field(stringify!($field), $field);
                            )*
                            dbg.finish()
                        },
                    )*
                }
            }
        }

        impl $crate::HasCore for $name {
            fn core(&self) -> &$crate::Core {
                match self {
                    $(
                        $name::$variant { core, .. } => core,
                    )*
                }
            }

            fn core_mut(&mut self) -> &mut $crate::Core {
                match self {
                    $(
                        $name::$variant { core, .. } => core,
                    )*
                }
            }
        }
    };

    (@inner
        $(#[$($type_attrs:meta)*])*
        $name:ident
        [
            $(
                (
                    $(#[$($variant_attrs:meta)*])*
                    $variant:ident,
                    $function:ident,
                    $counter:path,
                    $(#[$($core_attrs:meta)*])*
                    (
                        $(
                            $(#[$($field_attrs:meta)*])*
                            $field:ident: $field_type:path,
                        )*
                    )
                )
            )*
        ]
        $(#[$($new_variant_attrs:meta)*])*
        $new_variant:ident as $new_function:ident @ $new_counter:path {
            $(#[$($new_core_attrs:meta)*])*
            core,
            $(
                $(#[$($new_field_attrs:meta)*])*
                $new_field:ident: $new_field_type:path,
            )*
        },
        $($tt:tt)*
    ) => {
        handled!(
            @inner
            $(#[$($type_attrs)*])*
            $name
            [
                $(
                    (
                        $(#[$($variant_attrs)*])*
                        $variant,
                        $function,
                        $counter,
                        $(#[$($core_attrs)*])*
                        (
                            $(
                                $(#[$($field_attrs)*])*
                                $field: $field_type,
                            )*
                        )
                    )
                )*
                (
                    $(#[$($new_variant_attrs)*])*
                    $new_variant,
                    $new_function,
                    $new_counter,
                    $(#[$($new_core_attrs)*])*
                    (
                        $(
                            $(#[$($new_field_attrs)*])*
                            $new_field: $new_field_type,
                        )*
                    )
                )
            ]
            $($tt)*
        );
    }
}

////////////////////////////////////////////// handle //////////////////////////////////////////////

#[macro_export]
macro_rules! handle {
    ($call:expr, $($tt:tt)*) => {
        handle!(@inner $call ; [] [] $($tt)*)
    };

    (@inner $call:expr ; [$(($name:expr, $arg1:expr)),*] [$($arg2:expr),*]) => {
        ($call)($($arg2),*)
            $(.with_info($name, $arg1))*
    };

    (@inner $call:expr ; [$(($name:expr, $arg1:expr)),*] [$($arg2:expr),*] skip $new_arg:expr) => {
        handle!(@inner $call ; [$(($name, $arg1)),*] [$($arg2,)* $new_arg])
    };

    (@inner $call:expr ; [$(($name:expr, $arg1:expr)),*] [$($arg2:expr),*] skip $new_arg:expr, $($tt:expr)*) => {
        handle!(@inner $call ; [$(($name, $arg1)),*] [$($arg2,)* $new_arg] $($tt)*)
    };

    (@inner $call:expr ; [$(($name:expr, $arg1:expr)),*] [$($arg2:expr),*] $file:ident = $new_arg:expr) => {
        handle!(@inner $call ; [$(($name, $arg1),)* (stringify!($file), $new_arg)] [$($arg2,)* $new_arg])
    };

    (@inner $call:expr ; [$(($name:expr, $arg1:expr)),*] [$($arg2:expr),*] $file:ident = $new_arg:expr, $($tt:expr)*) => {
        handle!(@inner $call ; [$(($name, $arg1),)* (stringify!($file), $new_arg)] [$($arg2,)* $new_arg] $($tt)*)
    };

    (@inner $call:expr ; [$(($name:expr, $arg1:expr)),*] [$($arg2:expr),*] $new_arg:expr) => {
        handle!(@inner $call ; [$(($name, $arg1),)* (stringify!($new_arg), $new_arg)] [$($arg2,)* $new_arg])
    };

    (@inner $call:expr ; [$(($name:expr, $arg1:expr)),*] [$($arg2:expr),*] $new_arg:expr, $($tt:tt)*) => {
        handle!(@inner $call ; [$(($name, $arg1),)* (stringify!($new_arg), $new_arg)] [$($arg2,)* $new_arg] $($tt)*)
    };
}

////////////////////////////////////////// no_quote_debug //////////////////////////////////////////

pub fn no_quote_debug(s: &str) -> impl Debug {
    struct NoQuoteDebug(String);
    impl Debug for NoQuoteDebug {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "{}", self.0)
        }
    }
    NoQuoteDebug(s.to_string())
}
