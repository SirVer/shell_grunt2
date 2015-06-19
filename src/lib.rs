#![feature(scoped)]

pub mod task;
pub mod dispatch;

pub use task::Task;
pub use dispatch::loop_forever;
