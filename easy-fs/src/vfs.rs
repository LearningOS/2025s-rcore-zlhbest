use super::{
    block_cache_sync_all, get_block_cache, BlockDevice, DirEntry, DiskInode, DiskInodeType,
    EasyFileSystem, DIRENT_SZ,
};
use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;
use spin::{Mutex, MutexGuard};
/// Virtual filesystem layer over easy-fs
pub struct Inode {
    block_id: usize,
    block_offset: usize,
    // 作用就是记录下索引的inode具体的值
    inode_id: u32,
    fs: Arc<Mutex<EasyFileSystem>>,
    block_device: Arc<dyn BlockDevice>,
}

impl Inode {
    /// Create a vfs inode
    pub fn new(
        block_id: u32,
        block_offset: usize,
        inode_id: u32,
        fs: Arc<Mutex<EasyFileSystem>>,
        block_device: Arc<dyn BlockDevice>,
    ) -> Self {
        Self {
            block_id: block_id as usize,
            block_offset,
            inode_id,
            fs,
            block_device,
        }
    }
    /// Call a function over a disk inode to read it
    fn read_disk_inode<V>(&self, f: impl FnOnce(&DiskInode) -> V) -> V {
        get_block_cache(self.block_id, Arc::clone(&self.block_device))
            .lock()
            .read(self.block_offset, f)
    }
    /// Call a function over a disk inode to modify it
    fn modify_disk_inode<V>(&self, f: impl FnOnce(&mut DiskInode) -> V) -> V {
        get_block_cache(self.block_id, Arc::clone(&self.block_device))
            .lock()
            .modify(self.block_offset, f)
    }
    /// Find inode under a disk inode by name
    fn find_inode_id(&self, name: &str, disk_inode: &DiskInode) -> Option<u32> {
        // assert it is a directory
        assert!(disk_inode.is_dir());
        let file_count = (disk_inode.size as usize) / DIRENT_SZ;
        let mut dirent = DirEntry::empty();
        for i in 0..file_count {
            assert_eq!(
                disk_inode.read_at(DIRENT_SZ * i, dirent.as_bytes_mut(), &self.block_device,),
                DIRENT_SZ,
            );
            if dirent.name() == name {
                return Some(dirent.inode_id() as u32);
            }
        }
        None
    }
    /// 判断该索引索引的是文件还是目录
    pub fn is_file(&self) -> bool {
        self.read_disk_inode(|disk_inode| disk_inode.is_file())
    }
    /// 判断是不是目录
    pub fn is_dir(&self) -> bool {
        self.read_disk_inode(|disk_inode| disk_inode.is_dir())
    }
    /// Find inode under current inode by name
    pub fn find(&self, name: &str) -> Option<Arc<Inode>> {
        let fs = self.fs.lock();
        self.read_disk_inode(|disk_inode| {
            self.find_inode_id(name, disk_inode).map(|inode_id| {
                let (block_id, block_offset) = fs.get_disk_inode_pos(inode_id);
                Arc::new(Self::new(
                    block_id,
                    block_offset,
                    inode_id,
                    self.fs.clone(),
                    self.block_device.clone(),
                ))
            })
        })
    }
    /// Increase the size of a disk inode
    fn increase_size(
        &self,
        new_size: u32,
        disk_inode: &mut DiskInode,
        fs: &mut MutexGuard<EasyFileSystem>,
    ) {
        if new_size < disk_inode.size {
            return;
        }
        let blocks_needed = disk_inode.blocks_num_needed(new_size);
        let mut v: Vec<u32> = Vec::new();
        for _ in 0..blocks_needed {
            v.push(fs.alloc_data());
        }
        disk_inode.increase_size(new_size, v, &self.block_device);
    }
    ///  创建硬链接, 根据根目录创建文件
    /// 实现思路就是找出原来的Inode
    /// 然后根据根节点找到目录项，创建一个新的目录项，将目录项的inode_id改为旧的
    pub fn linkat(&self, old_name: &str, new_name: &str) {
        // 这里直接使用root是不合理的，后面再进行修改吧
        self.find(old_name).map(|old_name| {
            let mut fs = old_name.fs.lock();
            // 这里创建新的
            self.modify_disk_inode(|root_inode| {
                // append file in the dirent
                let file_count = (root_inode.size as usize) / DIRENT_SZ;
                let new_size = (file_count + 1) * DIRENT_SZ;
                // increase size
                self.increase_size(new_size as u32, root_inode, &mut fs);
                // write dirent 这里的create 创建的其实是一个目录项， 目录项中保存着索引的id
                let dirent = DirEntry::new(new_name, old_name.inode_id);
                root_inode.write_at(
                    file_count * DIRENT_SZ,
                    dirent.as_bytes(),
                    &self.block_device,
                );
            });
        });
    }
    /// 写一段代码检测一下存在多少的硬链接
    /// name 是用于查找具体是哪个文件的，因为我们所有的文件都是在根目录下只有一个，所以查找起来比较方便，直接遍历即可
    pub fn nlink(&self, inode: Arc<Inode>) -> usize {
        // 检测存在多少个
        let mut number = 0;
        self.read_disk_inode(|disk_inode| {
            let file_count = (disk_inode.size as usize) / DIRENT_SZ;
            for i in 0..file_count {
                let mut dirent = DirEntry::empty();
                assert_eq!(
                    disk_inode.read_at(i * DIRENT_SZ, dirent.as_bytes_mut(), &self.block_device,),
                    DIRENT_SZ,
                );
                // 这里进行对比 根据inode进行对比
                if dirent.inode_id() == inode.inode_id as u32 {
                    number += 1;
                }
            }
        });
        number
    }
    /// 删除硬链接，这里的self 就是根节点
    /// 返回的是剩余硬链接数
    pub fn unlinkat(&self, name: &str) -> isize {
        let inode = self.find(name);
        if inode.is_none() {
            return -1;
        }
        let inode = inode.unwrap();
        // 首先确认是不是只有一个
        let link_number = self.nlink(inode.clone());
        if link_number == 1 {
            // 这里的清空是清空的文件，但是并没有回收inode，其实这里可以不删除，只回收inode就可以了
            inode.clear();
            // 将索引清空掉
            self.fs.lock().dealloc_inode(inode.inode_id);
        }
        // 如果索引不止一个，那就仅仅删除掉目录项
        self.modify_disk_inode(|root_node| {
            // 这里主要是删除掉目录项,根节点存储这目录项
            let file_count = (root_node.size as usize) / DIRENT_SZ;
            let mut target_idx = None;
            let mut dirent = DirEntry::empty();
            // 找到需要修改的位置
            for i in 0..file_count {
                assert_eq!(
                    root_node.read_at(i * DIRENT_SZ, dirent.as_bytes_mut(), &self.block_device),
                    DIRENT_SZ,
                );
                if dirent.name() == name {
                    target_idx = Some(i);
                    break;
                }
            }
            // 进行修改
            if let Some(idx) = target_idx {
                // 如果不是最后一个，那就需要吧最后一个塞到要删除的地方
                if idx != file_count - 1 {
                    // 用最后一个覆盖
                    let mut last_dirent = DirEntry::empty();
                    assert_eq!(
                        root_node.read_at(
                            (file_count - 1) * DIRENT_SZ,
                            last_dirent.as_bytes_mut(),
                            &self.block_device
                        ),
                        DIRENT_SZ,
                    );
                    root_node.write_at(idx * DIRENT_SZ, last_dirent.as_bytes(), &self.block_device);
                }
                // 缩小目录文件大小，不管是不是最后一个，只需要缩小size即可
                root_node.size -= DIRENT_SZ as u32;
            }
        });
        // 返回还剩下几个
        (link_number - 1) as isize
    }
    /// Create inode under current inode by name
    pub fn create(&self, name: &str) -> Option<Arc<Inode>> {
        let mut fs = self.fs.lock();
        let op = |root_inode: &DiskInode| {
            // assert it is a directory
            assert!(root_inode.is_dir());
            // has the file been created?
            self.find_inode_id(name, root_inode)
        };
        if self.read_disk_inode(op).is_some() {
            return None;
        }
        // create a new file
        // alloc a inode with an indirect block 分配一个索引块
        let new_inode_id = fs.alloc_inode();
        // initialize inode
        let (new_inode_block_id, new_inode_block_offset) = fs.get_disk_inode_pos(new_inode_id);
        get_block_cache(new_inode_block_id as usize, Arc::clone(&self.block_device))
            .lock()
            .modify(new_inode_block_offset, |new_inode: &mut DiskInode| {
                new_inode.initialize(DiskInodeType::File);
            });
        self.modify_disk_inode(|root_inode| {
            // append file in the dirent
            let file_count = (root_inode.size as usize) / DIRENT_SZ;
            let new_size = (file_count + 1) * DIRENT_SZ;
            // increase size
            self.increase_size(new_size as u32, root_inode, &mut fs);
            // write dirent 这里的create 创建的其实是一个目录项， 目录项中保存着索引的id
            let dirent = DirEntry::new(name, new_inode_id);
            root_inode.write_at(
                file_count * DIRENT_SZ,
                dirent.as_bytes(),
                &self.block_device,
            );
        });
        // 由索引块计算出block_id和block_offset
        let (block_id, block_offset) = fs.get_disk_inode_pos(new_inode_id);
        block_cache_sync_all();
        // return inode
        Some(Arc::new(Self::new(
            block_id,
            block_offset,
            new_inode_id,
            self.fs.clone(),
            self.block_device.clone(),
        )))
        // release efs lock automatically by compiler
    }
    /// List inodes under current inode
    pub fn ls(&self) -> Vec<String> {
        let _fs = self.fs.lock();
        self.read_disk_inode(|disk_inode| {
            let file_count = (disk_inode.size as usize) / DIRENT_SZ;
            let mut v: Vec<String> = Vec::new();
            for i in 0..file_count {
                let mut dirent = DirEntry::empty();
                assert_eq!(
                    disk_inode.read_at(i * DIRENT_SZ, dirent.as_bytes_mut(), &self.block_device,),
                    DIRENT_SZ,
                );
                v.push(String::from(dirent.name()));
            }
            v
        })
    }
    /// Read data from current inode
    pub fn read_at(&self, offset: usize, buf: &mut [u8]) -> usize {
        let _fs = self.fs.lock();
        self.read_disk_inode(|disk_inode| disk_inode.read_at(offset, buf, &self.block_device))
    }
    /// Write data to current inode
    pub fn write_at(&self, offset: usize, buf: &[u8]) -> usize {
        let mut fs = self.fs.lock();
        let size = self.modify_disk_inode(|disk_inode| {
            self.increase_size((offset + buf.len()) as u32, disk_inode, &mut fs);
            disk_inode.write_at(offset, buf, &self.block_device)
        });
        block_cache_sync_all();
        size
    }
    /// Clear the data in current inode
    /// 但是这个清空并不能清空掉目录项，也不会清空掉索引
    pub fn clear(&self) {
        let mut fs = self.fs.lock();
        self.modify_disk_inode(|disk_inode| {
            let size = disk_inode.size;
            // 将inode的内容都变成0 然后返回需要清空的
            let data_blocks_dealloc = disk_inode.clear_size(&self.block_device);
            assert!(data_blocks_dealloc.len() == DiskInode::total_blocks(size) as usize);
            //清空所有的数据
            for data_block in data_blocks_dealloc.into_iter() {
                fs.dealloc_data(data_block);
            }
        });
        block_cache_sync_all();
    }
    /// 获取inode_id
    pub fn get_inode_id(&self) -> u32 {
        self.inode_id
    }
}
