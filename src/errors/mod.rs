pub mod api;
pub mod fetcher;

// to delete '$into_type:path' you need to use proc macros and further manipulation of the AST
#[macro_export]
macro_rules! error_from {
    (move $from:path, $into_type:path, $into:path) => {
        impl From<$from> for $into_type {
            fn from(err: $from) -> Self {
                $into(err)
            }
        }
    };
    (replace $from:path, $into_type:path, $into:path) => {
        impl From<$from> for $into_type {
            fn from(_: $from) -> Self {
                $into
            }
        }
    };
    (transform $from:path, $into_type:path, |$err_name:ident| $blk:block) => {
        impl From<$from> for $into_type {
            fn from($err_name: $from) -> Self {
                $blk
            }
        }
    };
    (transform_io $from:path, $into_type:path) => {
        impl From<$from> for $into_type {
            fn from(err: $from) -> Self {
                std::io::Error::from(err).into()
            }
        }
    };
}
