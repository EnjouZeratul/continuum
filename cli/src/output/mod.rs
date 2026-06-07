//! 输出处理模块

pub mod format;
pub mod loader;
pub mod streaming;
pub mod terminal_colors;

/// 输出处理器 trait
pub trait OutputHandler: Send + Sync {
    fn handle(&self, output: &str) -> anyhow::Result<()>;
}

/// 默认输出处理器
pub struct DefaultOutputHandler;

impl OutputHandler for DefaultOutputHandler {
    fn handle(&self, output: &str) -> anyhow::Result<()> {
        println!("{}", output);
        Ok(())
    }
}
