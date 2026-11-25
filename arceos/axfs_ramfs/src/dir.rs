use alloc::collections::BTreeMap;
use alloc::sync::{Arc, Weak};
use alloc::{string::String, vec::Vec};

use axfs_vfs::{VfsDirEntry, VfsNodeAttr, VfsNodeOps, VfsNodeRef, VfsNodeType};
use axfs_vfs::{VfsError, VfsResult};
use spin::RwLock;

use crate::file::FileNode;

/// The directory node in the RAM filesystem.
///
/// It implements [`axfs_vfs::VfsNodeOps`].
pub struct DirNode {
    this: Weak<DirNode>,
    parent: RwLock<Weak<dyn VfsNodeOps>>,
    children: RwLock<BTreeMap<String, VfsNodeRef>>,
}

impl DirNode {
    pub(super) fn new(parent: Option<Weak<dyn VfsNodeOps>>) -> Arc<Self> {
        Arc::new_cyclic(|this| Self {
            this: this.clone(),
            parent: RwLock::new(parent.unwrap_or_else(|| Weak::<Self>::new())),
            children: RwLock::new(BTreeMap::new()),
        })
    }

    pub(super) fn set_parent(&self, parent: Option<&VfsNodeRef>) {
        *self.parent.write() = parent.map_or(Weak::<Self>::new() as _, Arc::downgrade);
    }

    /// Returns a string list of all entries in this directory.
    pub fn get_entries(&self) -> Vec<String> {
        self.children.read().keys().cloned().collect()
    }

    /// Checks whether a node with the given name exists in this directory.
    pub fn exist(&self, name: &str) -> bool {
        self.children.read().contains_key(name)
    }

    /// Creates a new node with the given name and type in this directory.
    pub fn create_node(&self, name: &str, ty: VfsNodeType) -> VfsResult {
        if self.exist(name) {
            log::error!("AlreadyExists {}", name);
            return Err(VfsError::AlreadyExists);
        }
        let node: VfsNodeRef = match ty {
            VfsNodeType::File => Arc::new(FileNode::new()),
            VfsNodeType::Dir => Self::new(Some(self.this.clone())),
            _ => return Err(VfsError::Unsupported),
        };
        self.children.write().insert(name.into(), node);
        Ok(())
    }

    /// Removes a node by the given name in this directory.
    pub fn remove_node(&self, name: &str) -> VfsResult {
        let mut children = self.children.write();
        let node = children.get(name).ok_or(VfsError::NotFound)?;
        if let Some(dir) = node.as_any().downcast_ref::<DirNode>() {
            if !dir.children.read().is_empty() {
                return Err(VfsError::DirectoryNotEmpty);
            }
        }
        children.remove(name);
        Ok(())
    }

    // ****** START xisanlou add at  25102401 No.1
    /// 需要能从外部访问childen
    pub(crate) fn get_children_node(&self, name: &str) -> VfsResult<VfsNodeRef> {
        self.children.read().get(name).cloned().ok_or(VfsError::NotFound)      
    } 

    /// 获取this的值
    pub(crate) fn this(&self) -> Weak<DirNode> {
        self.this.clone()
    }
    
    /// 添加给定名称的节点到目录
    pub(crate) fn add_node_in_childen(&self, name: &str, node: VfsNodeRef) {
        let mut children = self.children.write();
        children.insert(String::from(name), node);
    }

    /// 内部重命名：源文件与目标文件都是短文件名
    fn rename_internal(
        &self,
        src_name: &str,
        dst_dir: Weak<DirNode>,
        dst_name: &str,
    ) -> VfsResult {
        
        // 查找并确认源文件或目录存在
        if ! self.exist(src_name) {return Err(VfsError::NotFound);}

        // 将dst_dir提升为Arc
        let dst_dir = dst_dir.upgrade().ok_or(VfsError::PermissionDenied)?;

        // 检查目标文件名是否已经存在
        if dst_dir.exist(dst_name) {
            // 如果目标存在，检查是否与源文件是同一目录和同一文件，直接返回Ok,不同则返回error
            
            if self.this().ptr_eq(&dst_dir.this()) && src_name == dst_name {
                return Ok(());
            } else {
                return Err(VfsError::AlreadyExists);
            }
        }
        
        // 从源目录删除"src_name对象"，并将删除的对象返回
        // 取回目录项列表
        let mut src_children = self.children.write();
        
        // 从元目录删除对象
        let node = src_children.remove(src_name).ok_or(VfsError::PermissionDenied)?;
        // 如果"src_name对象"是一个目录，则更新parent值为dst_dir
        if let Some(sub_dir) = node.as_any().downcast_ref::<DirNode>() {
            let dst_ref: VfsNodeRef = dst_dir.this().upgrade().ok_or(VfsError::PermissionDenied)?;
            sub_dir.set_parent(Some(&dst_ref));
        }

        drop(src_children);
        
        // 将节点添加到目标目录
        dst_dir.add_node_in_childen(dst_name, node);
        
        Ok(())
    }

    /// 重命名函数，参数包含目标目录结构
    pub(crate) fn rename3(&self, src_path: &str, dst_dir: Weak<DirNode>, dst_path: &str) -> VfsResult {
        //if src_path != "/f1" { return Err(VfsError::StorageFull);} 
        // traverse source path
        let (src_name, src_rest_opt) = split_path(src_path);
        let src_node = match src_name {
            "" | "." => Ok(self.this().upgrade().ok_or(VfsError::NotFound)? as VfsNodeRef),
            ".." => self.parent().ok_or(VfsError::NotFound),
            _ => self
                .children
                .read()
                .get(src_name)
                .cloned()
                .ok_or(VfsError::NotFound),
        }?;
        if let Some(rest) = src_rest_opt {
            // 检查node是否能转换为目录。如果是，则递归调用rename3；否，返回错误
            if let Some(dir) = src_node.as_any().downcast_ref::<DirNode>() {
                return dir.rename3(rest, dst_dir, dst_path);
            } else {
                return Err(VfsError::NotADirectory);
            }   
        }
        // traverse destination path
        let (dst_name, dst_rest_opt) = split_path(dst_path);
            
        if let Some(rest) = dst_rest_opt {
            // 只有在目录，dst_node 才存在
            let dst_node = match dst_name {
                "" | "." => Ok(dst_dir.upgrade().ok_or(VfsError::NotFound)? as VfsNodeRef),
                ".." => dst_dir.upgrade().ok_or(VfsError::NotFound)?.parent().ok_or(VfsError::NotFound),
                _ => dst_dir.upgrade().ok_or(VfsError::NotFound)?
                    .get_children_node(dst_name),
                //_ => Err(VfsError::ConnectionReset),
            }?;
            // 检查node是否能转换为目录。如果是，则递归调用rename3；否，返回错误
            if let Some(dir) = dst_node.as_any().downcast_ref::<DirNode>() {
                return self.rename3(src_path, dir.this(), rest);
            } else {
                return Err(VfsError::NotADirectory);
            } 
        }
        // move/rename file
        self.rename_internal(src_path, dst_dir, dst_path)
    }
    // ****** END   xisanlou add at 25102401 No.1
}

impl VfsNodeOps for DirNode {
    fn get_attr(&self) -> VfsResult<VfsNodeAttr> {
        Ok(VfsNodeAttr::new_dir(4096, 0))
    }

    fn parent(&self) -> Option<VfsNodeRef> {
        self.parent.read().upgrade()
    }

    fn lookup(self: Arc<Self>, path: &str) -> VfsResult<VfsNodeRef> {
        let (name, rest) = split_path(path);
        let node = match name {
            "" | "." => Ok(self.clone() as VfsNodeRef),
            ".." => self.parent().ok_or(VfsError::NotFound),
            _ => self
                .children
                .read()
                .get(name)
                .cloned()
                .ok_or(VfsError::NotFound),
        }?;

        if let Some(rest) = rest {
            node.lookup(rest)
        } else {
            Ok(node)
        }
    }

    fn read_dir(&self, start_idx: usize, dirents: &mut [VfsDirEntry]) -> VfsResult<usize> {
        let children = self.children.read();
        let mut children = children.iter().skip(start_idx.max(2) - 2);
        for (i, ent) in dirents.iter_mut().enumerate() {
            match i + start_idx {
                0 => *ent = VfsDirEntry::new(".", VfsNodeType::Dir),
                1 => *ent = VfsDirEntry::new("..", VfsNodeType::Dir),
                _ => {
                    if let Some((name, node)) = children.next() {
                        *ent = VfsDirEntry::new(name, node.get_attr().unwrap().file_type());
                    } else {
                        return Ok(i);
                    }
                }
            }
        }
        Ok(dirents.len())
    }

    fn create(&self, path: &str, ty: VfsNodeType) -> VfsResult {
        log::debug!("create {:?} at ramfs: {}", ty, path);
        let (name, rest) = split_path(path);
        if let Some(rest) = rest {
            match name {
                "" | "." => self.create(rest, ty),
                ".." => self.parent().ok_or(VfsError::NotFound)?.create(rest, ty),
                _ => {
                    let subdir = self
                        .children
                        .read()
                        .get(name)
                        .ok_or(VfsError::NotFound)?
                        .clone();
                    subdir.create(rest, ty)
                }
            }
        } else if name.is_empty() || name == "." || name == ".." {
            Ok(()) // already exists
        } else {
            self.create_node(name, ty)
        }
    }

    fn remove(&self, path: &str) -> VfsResult {
        log::debug!("remove at ramfs: {}", path);
        let (name, rest) = split_path(path);
        if let Some(rest) = rest {
            match name {
                "" | "." => self.remove(rest),
                ".." => self.parent().ok_or(VfsError::NotFound)?.remove(rest),
                _ => {
                    let subdir = self
                        .children
                        .read()
                        .get(name)
                        .ok_or(VfsError::NotFound)?
                        .clone();
                    subdir.remove(rest)
                }
            }
        } else if name.is_empty() || name == "." || name == ".." {
            Err(VfsError::InvalidInput) // remove '.' or '..
        } else {
            self.remove_node(name)
        }
    }

    fn rename(&self, _src_path: &str, _dst_path: &str) -> VfsResult {
        log::info!("Haogy Add: Call rename3!");
        self.rename3(_src_path, self.this(), _dst_path)
    }

    axfs_vfs::impl_vfs_dir_default! {}
}

fn split_path(path: &str) -> (&str, Option<&str>) {
    let trimmed_path = path.trim_start_matches('/');
    trimmed_path.find('/').map_or((trimmed_path, None), |n| {
        (&trimmed_path[..n], Some(&trimmed_path[n + 1..]))
    })
}
