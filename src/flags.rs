macro_rules! flags {
    {
        $struct_vis:vis struct $struct_name:ident($type:ty) {
            $(
                $(#[doc = $doc:expr])?
                $field_vis:vis $field_name:ident = $field_value:expr;
            )*
        }
    } => {
        #[derive(Clone, Copy)]
        $struct_vis struct $struct_name($type);

        impl $struct_name {
            pub fn new(flags: $type) -> Self {
                Self(flags)
            }

            $(
                $(#[doc = $doc])?
                $field_vis fn $field_name(&self) -> bool {
                    self.0 & $field_value != 0
                }
            )*
        }

        impl std::fmt::Debug for $struct_name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.debug_struct(stringify!($struct_name))
                    $(
                        .field(stringify!($field_name), &self.$field_name())
                    )*
                    .finish()
            }
        }
    }
}
