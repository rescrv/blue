pub mod id;
pub mod stopwatch;

#[macro_export]
macro_rules! generate_id {
    ($what:ident, $prefix:literal) => {
        #[derive(Debug, Eq, PartialEq, PartialOrd, Ord, Clone, Copy, Hash)]
        pub struct $what {
            id: [u8; util::id::BYTES],
        }

        impl $what {
            pub const BOTTOM: $what = $what { id: [0u8; util::id::BYTES] };
            pub const TOP: $what = $what { id: [0xffu8; util::id::BYTES], };

            pub fn generate() -> Option<$what> {
                match util::id::urandom() {
                    Some(id) => Some($what { id }),
                    None => None
                }
            }

            pub fn from_human_readable(s: &str) -> Option<Self> {
                let prefix = $prefix;
                if !s.starts_with(prefix) {
                    return None;
                }
                match util::id::decode(&s[prefix.len()..]) {
                    Some(x) => Some(Self::new(x)),
                    None => None,
                }
            }

            pub fn human_readable(&self) -> String {
                let readable = $prefix.to_string();
                readable + &util::id::encode(&self.id)
            }

            pub fn prefix_free_readable(&self) -> String {
                util::id::encode(&self.id)
            }

            fn new(id: [u8; util::id::BYTES]) -> Self {
                Self {
                    id
                }
            }
        }

        impl Default for $what {
            fn default() -> $what {
                $what::BOTTOM
            }
        }

        impl std::fmt::Display for $what {
            fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                write!(f, "{}{}", $prefix, util::id::encode(&self.id))
            }
        }
    }
}
