#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Selector(String);

impl Selector {
    pub fn new() -> Self {
        Self(String::with_capacity(16))
    }

    pub fn unary(op: &str) -> Self {
        Self(op.to_string())
    }

    pub fn push(&self, keyword: &str) -> Self {
        let mut selector = self.clone();
        selector.0.push_str(keyword);
        selector.0.push(':');
        selector
    }
}

impl std::fmt::Display for Selector {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}
