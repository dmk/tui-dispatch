use std::fmt::Debug;

pub fn debug_string<T>(value: &T) -> String
where
    T: Debug,
{
    debug_string_compact(value)
}

pub fn debug_string_compact<T>(value: &T) -> String
where
    T: Debug,
{
    format!("{:?}", value)
}

pub fn debug_string_pretty<T>(value: &T) -> String
where
    T: Debug,
{
    format!("{:#?}", value)
}
