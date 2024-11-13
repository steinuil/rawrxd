/// Automatically implement TryFrom<T> for the repr(T) of the enum.
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
        #[repr($type)]
        $(#[doc = $struct_doc])*
        $vis enum $name {
            $(
                $(#[doc = $field_doc])*
                $field_name = $field_value,
            )*
        }

        impl TryFrom<$type> for $name {
            type Error = $type;

            fn try_from(value: $type) -> Result<Self, Self::Error> {
                match value {
                    $(
                        v if v == $name::$field_name as $type => Ok($name::$field_name),
                    )*
                    _ => Err(value),
                }
            }
        }
    };
}
