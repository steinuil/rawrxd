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
            let $record:ident {
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
