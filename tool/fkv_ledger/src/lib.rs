//! fkv 迁移账本库：tree-sitter 扫描 eloqstore，motedb 存储，clap_args 认领。
//!
//! CLI 入口在 `main.rs`；集成测试直接使用本 crate 的公开 API，
//! 不再用 `#[path]` 把源码并入测试 crate。

pub mod adopt;
pub mod baseline;
pub mod classify;
pub mod corpus;
pub mod db;
pub mod error;
pub mod fixture;
pub mod ledger;
pub mod map;
pub mod scan;
pub mod util;
