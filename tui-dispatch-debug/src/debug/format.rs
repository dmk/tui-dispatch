use std::fmt::Debug;

pub fn ron_string<T>(value: &T) -> String
where
    T: Debug,
{
    ron_string_compact(value)
}

pub fn ron_string_compact<T>(value: &T) -> String
where
    T: Debug,
{
    format!("{:?}", value)
}

pub fn ron_string_pretty<T>(value: &T) -> String
where
    T: Debug,
{
    format!("{:#?}", value)
}
