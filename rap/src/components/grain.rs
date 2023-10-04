use std::fmt::{Display, Formatter};

#[derive(Debug, Copy, Clone, Hash, Ord, PartialOrd, Eq, PartialEq)]
pub enum RapGrain {
    Low = 0,
    Medium = 1,
    High = 2,
    Ultra = 3,
}

impl Display for RapGrain {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                RapGrain::Ultra => "Ultra",
                RapGrain::High => "High",
                RapGrain::Medium => "Medium",
                RapGrain::Low => "Low,"
            }
        )
    }
}