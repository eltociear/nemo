//! This module defines logical data structures and operations.

pub mod permutator;

pub mod builder_proxy;
pub use builder_proxy::LogicalColumnBuilderProxy;

pub use permutator::Permutator;

pub mod model;

pub mod program_analysis;

pub mod execution;

pub mod table_manager;
pub use table_manager::TableManager;

pub mod types;

pub mod util;
