use serde::Serialize;
use std::fmt::Debug;

pub fn ron_string<T>(value: &T) -> String
where
    T: Serialize + Debug,
{
    ron_string_compact(value)
}

pub fn ron_string_compact<T>(value: &T) -> String
where
    T: Serialize + Debug,
{
    let pretty = ron::ser::PrettyConfig::new()
        .struct_names(true)
        .compact_arrays(true)
        .separate_tuple_members(false);

    match ron::ser::to_string_pretty(value, pretty) {
        Ok(serialized) => serialized,
        Err(_) => format!("{:?}", value),
    }
}

pub fn ron_string_pretty<T>(value: &T) -> String
where
    T: Serialize + Debug,
{
    let pretty = ron::ser::PrettyConfig::new()
        .struct_names(true)
        .compact_arrays(false)
        .separate_tuple_members(true);

    match ron::ser::to_string_pretty(value, pretty) {
        Ok(serialized) => serialized,
        Err(_) => format!("{:?}", value),
    }
}
