use std::str::FromStr;

use clap::ValueEnum;
use serde::{Deserialize, Serialize};

macro_rules! impl_from_str_via_value_enum {
    ($ty:ty) => {
        impl FromStr for $ty {
            type Err = String;
            fn from_str(s: &str) -> Result<Self, Self::Err> {
                <$ty as ValueEnum>::from_str(s, true)
            }
        }
    };
}

/// Single source of truth for `--on-error`, shared by fsapp's clap definitions
/// and fset's validation — see fsapp-design-spec.md §5.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[clap(rename_all = "kebab-case")]
pub enum OnError {
    Continue,
    Abort,
    Undo,
}

impl Default for OnError {
    fn default() -> Self {
        OnError::Continue
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[clap(rename_all = "kebab-case")]
pub enum SortOrder {
    Asc,
    Desc,
}

impl Default for SortOrder {
    fn default() -> Self {
        SortOrder::Desc
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[clap(rename_all = "kebab-case")]
pub enum CompressFormat {
    Zip,
    Gzip,
}

impl_from_str_via_value_enum!(OnError);
impl_from_str_via_value_enum!(SortOrder);
impl_from_str_via_value_enum!(CompressFormat);
