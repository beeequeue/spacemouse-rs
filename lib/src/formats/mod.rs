use core::fmt;

pub mod v0;
pub mod v1;

#[derive(PartialEq, Eq, Clone, Copy)]
pub enum Format {
    V0,
    V1,
}

impl fmt::Display for Format {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::V0 => write!(f, "Original"),
            Self::V1 => write!(f, "Current"),
        }
    }
}
