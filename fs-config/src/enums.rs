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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn on_error_parses_kebab_case_values() {
        assert_eq!("continue".parse::<OnError>().unwrap(), OnError::Continue);
        assert_eq!("abort".parse::<OnError>().unwrap(), OnError::Abort);
        assert_eq!("undo".parse::<OnError>().unwrap(), OnError::Undo);
    }

    #[test]
    fn on_error_parsing_is_case_insensitive() {
        assert_eq!("Continue".parse::<OnError>().unwrap(), OnError::Continue);
        assert_eq!("ABORT".parse::<OnError>().unwrap(), OnError::Abort);
    }

    #[test]
    fn on_error_rejects_unknown_values() {
        assert!("retry".parse::<OnError>().is_err());
    }

    #[test]
    fn sort_order_round_trips_through_json() {
        let json = serde_json::to_string(&SortOrder::Asc).unwrap();
        assert_eq!(json, "\"asc\"");
        assert_eq!(serde_json::from_str::<SortOrder>(&json).unwrap(), SortOrder::Asc);
    }

    #[test]
    fn compress_format_parses_kebab_case_values() {
        assert_eq!("zip".parse::<CompressFormat>().unwrap(), CompressFormat::Zip);
        assert_eq!("gzip".parse::<CompressFormat>().unwrap(), CompressFormat::Gzip);
        assert!("tar".parse::<CompressFormat>().is_err());
    }

    #[test]
    fn defaults_match_file_engine_builder_defaults() {
        // §6.3: absent keys mean "builder default" — these are the values
        // the builders themselves default to, so `Default` must agree.
        assert_eq!(OnError::default(), OnError::Continue);
        assert_eq!(SortOrder::default(), SortOrder::Desc);
    }
}
