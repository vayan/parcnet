#[macro_export]
macro_rules! pod_entries {
    ($($key:expr => $value:expr),* $(,)?) => {{
        vec![
            $(
                (String::from($key), $crate::pod::PodValue::from($value)),
            )*
        ]
    }};
}
