/// Automatically implement From<T> for the type of the enum
/// and add an Unknown case for when we fail parsing.
///
/// # Syntax
///
/// ```
/// int_enum! {
///     pub enum EnumName : type {
///         Case1 = 0x01,
///         Case2 = 0x02,
///     }
/// }
/// ```
macro_rules! int_enum {
    {
        $(#[doc = $struct_doc:expr])*
        $vis:vis enum $name:ident : $type:ty {
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

macro_rules! flags {
    {
        $(#[doc = $struct_doc:expr])*
        $struct_vis:vis struct $struct_name:ident($type:ty) {
            $(
                $(#[doc = $field_doc:expr])*
                $field_vis:vis $field_name:ident = $field_value:expr;
            )*
        }
    } => {
        #[derive(Clone, Copy)]
        $(#[doc = $struct_doc])*
        $struct_vis struct $struct_name($type);

        impl $struct_name {
            pub fn new(flags: $type) -> Self {
                Self(flags)
            }

            $(
                $(#[doc = $field_doc])*
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

/// Helper macro to parse RAR50 records.
macro_rules! parse_records {
    {
        $reader:expr,
        $common_header:expr,
        $unknown:ident,
        let {
            $(
                $var_name:ident: $struct_name:ident = $tag:pat,
            )*
        }

        $(
            match $record:ident {
                $(
                    $extra_tag:pat => $extra_block:block
                )*
            }
        )?
    } => {
        $(
            let mut $var_name = None;
        )*
        let mut $unknown = vec![];

        if let Some(extra_area_size) = $common_header.extra_area_size {
            for record in RecordIterator::new($reader, extra_area_size)? {
                let mut record = record?;

                match record.record_type {
                    $(
                        $tag if $var_name.is_none() => {
                            $var_name = Some($struct_name::read(&mut record.data)?)
                        }
                    )*
                    $(
                        $(
                            $extra_tag => {
                                let mut $record = record;
                                $extra_block
                            }
                        )*
                    )?
                    _ => $unknown.push(UnknownRecord::new(record.record_type)),
                }
            }
        }
    }
}
