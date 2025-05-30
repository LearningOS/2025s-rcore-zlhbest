//!An easy file system isolated from the kernel
#![no_std]
#![deny(missing_docs)]
extern crate alloc;
// 位图抽象
mod bitmap;
// 块缓存
mod block_cache;
// 声明块设备抽象接口 需要库的使用者提供实现
mod block_dev;
// 实现整个EasyFileSystem 的磁盘布局
mod efs;
// 保存在磁盘上的数据结构
mod layout;
mod vfs;
/// Use a block size of 512 bytes
pub const BLOCK_SZ: usize = 512;
use bitmap::Bitmap;
use block_cache::{block_cache_sync_all, get_block_cache};
pub use block_dev::BlockDevice;
pub use efs::EasyFileSystem;
use layout::*;
pub use vfs::Inode;
