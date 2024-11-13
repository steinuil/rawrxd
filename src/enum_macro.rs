/// Automatically implement From<T> for the repr(T) of the enum
/// and add an Unknown case for when we fail parsing.
macro_rules! int_enum {
    {
        $(#[doc = $struct_doc:expr])*
        #[repr($type:ty)]
        $vis:vis enum $name:ident {
            $(
                $(#[doc = $field_doc:expr])*
                $field_name:ident = $field_value:expr,
            )*
        }
    } => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        $(#[doc = $struct_doc])*
        $vis enum $name {
            $(
                $(#[doc = $field_doc])*
                $field_name,
            )*
            Unknown($type),
        }

        impl From<$type> for $name {
            fn from(value: $type) -> Self {
                match value {
                    $(
                        $field_value => $name::$field_name,
                    )*
                    _ => $name::Unknown(value),
                }

            }
        }
    };
}
