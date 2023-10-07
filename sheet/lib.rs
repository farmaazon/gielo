pub mod parameters;
pub mod stone;
pub use parameters::Parameters;

#[derive(Copy, Clone, Debug, Default, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub enum Hack {
    #[default]
    Left,
    Right,
}
