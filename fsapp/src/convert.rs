//! `fs_config`'s CLI/config-facing enums (kebab-case, `Asc`/`Desc`,
//! `Continue`/`Abort`/`Undo`) are deliberately distinct types from
//! `file_engine`'s builder-facing enums (`Ascending`/`Descending`,
//! `ContinueAndCollect`/`AbortOnError`/`Undo`) — this is the one place
//! that bridges them.

pub fn to_error_strategy(v: fs_config::OnError) -> file_engine::ErrorStrategy {
    match v {
        fs_config::OnError::Continue => file_engine::ErrorStrategy::ContinueAndCollect,
        fs_config::OnError::Abort => file_engine::ErrorStrategy::AbortOnError,
        fs_config::OnError::Undo => file_engine::ErrorStrategy::Undo,
    }
}

pub fn to_sort_order(v: fs_config::SortOrder) -> file_engine::SortOrder {
    match v {
        fs_config::SortOrder::Asc => file_engine::SortOrder::Ascending,
        fs_config::SortOrder::Desc => file_engine::SortOrder::Descending,
    }
}

pub fn to_compress_format(v: fs_config::CompressFormat) -> file_engine::CompressFormat {
    match v {
        fs_config::CompressFormat::Zip => file_engine::CompressFormat::Zip,
        fs_config::CompressFormat::Gzip => file_engine::CompressFormat::Gzip,
    }
}
