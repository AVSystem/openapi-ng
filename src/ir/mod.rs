pub(crate) mod canonical;
pub(crate) mod identifier;
pub(crate) mod normalize;
pub(crate) mod schema;

#[cfg(test)]
mod tests;

pub(crate) use normalize::normalize_api_model;
