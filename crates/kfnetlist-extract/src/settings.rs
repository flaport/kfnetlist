//! Domain encoding with caller-owned opaque values at the language boundary.
use indexmap::IndexMap;
#[derive(Debug)]
pub enum Setting<T> {
    Map(IndexMap<String, Setting<T>>),
    List(Vec<Setting<T>>),
    Tuple(Vec<Setting<T>>),
    Native { class: String, text: String },
    Encoded(String),
    Opaque(T),
}
impl<T> Setting<T> {
    pub fn encode(self) -> Self {
        match self {
            Self::Map(values) => {
                Self::Map(values.into_iter().map(|(k, v)| (k, v.encode())).collect())
            }
            Self::List(values) => Self::List(values.into_iter().map(Self::encode).collect()),
            Self::Tuple(values) => Self::Tuple(values.into_iter().map(Self::encode).collect()),
            Self::Native { class, text } => Self::Encoded(format!("!#{class} {text}")),
            other => other,
        }
    }
}
