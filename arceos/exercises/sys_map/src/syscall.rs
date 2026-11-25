#![allow(dead_code)]

use core::ffi::{c_void, c_char, c_int};
use axhal::arch::TrapFrame;
use axhal::trap::{register_trap_handler, SYSCALL};
use axerrno::LinuxError;
use axtask::current;
use axtask::TaskExtRef;
use axhal::paging::MappingFlags;
use arceos_posix_api as api;

// ****** START xisanlou add at  25122606 No.1
use memory_addr::{is_aligned_4k, VirtAddr, VirtAddrRange, align_up_4k};
use alloc::vec::Vec;

use axhal::mem::phys_to_virt;
// ****** END   xisanlou add at 25122606 No.1

const SYS_IOCTL: usize = 29;
const SYS_OPENAT: usize = 56;
const SYS_CLOSE: usize = 57;
const SYS_READ: usize = 63;
const SYS_WRITE: usize = 64;
const SYS_WRITEV: usize = 66;
const SYS_EXIT: usize = 93;
const SYS_EXIT_GROUP: usize = 94;
const SYS_SET_TID_ADDRESS: usize = 96;
const SYS_MMAP: usize = 222;

const AT_FDCWD: i32 = -100;

/// Macro to generate syscall body
///
/// It will receive a function which return Result<_, LinuxError> and convert it to
/// the type which is specified by the caller.
#[macro_export]
macro_rules! syscall_body {
    ($fn: ident, $($stmt: tt)*) => {{
        #[allow(clippy::redundant_closure_call)]
        let res = (|| -> axerrno::LinuxResult<_> { $($stmt)* })();
        match res {
            Ok(_) | Err(axerrno::LinuxError::EAGAIN) => debug!(concat!(stringify!($fn), " => {:?}"),  res),
            Err(_) => info!(concat!(stringify!($fn), " => {:?}"), res),
        }
        match res {
            Ok(v) => v as _,
            Err(e) => {
                -e.code() as _
            }
        }
    }};
}

bitflags::bitflags! {
    #[derive(Debug)]
    /// permissions for sys_mmap
    ///
    /// See <https://github.com/bminor/glibc/blob/master/bits/mman.h>
    struct MmapProt: i32 {
        /// Page can be read.
        const PROT_READ = 1 << 0;
        /// Page can be written.
        const PROT_WRITE = 1 << 1;
        /// Page can be executed.
        const PROT_EXEC = 1 << 2;
    }
}

impl From<MmapProt> for MappingFlags {
    fn from(value: MmapProt) -> Self {
        let mut flags = MappingFlags::USER;
        if value.contains(MmapProt::PROT_READ) {
            flags |= MappingFlags::READ;
        }
        if value.contains(MmapProt::PROT_WRITE) {
            flags |= MappingFlags::WRITE;
        }
        if value.contains(MmapProt::PROT_EXEC) {
            flags |= MappingFlags::EXECUTE;
        }
        flags
    }
}

bitflags::bitflags! {
    #[derive(Debug)]
    /// flags for sys_mmap
    ///
    /// See <https://github.com/bminor/glibc/blob/master/bits/mman.h>
    struct MmapFlags: i32 {
        /// Share changes
        const MAP_SHARED = 1 << 0;
        /// Changes private; copy pages on write.
        const MAP_PRIVATE = 1 << 1;
        /// Map address must be exactly as requested, no matter whether it is available.
        const MAP_FIXED = 1 << 4;
        /// Don't use a file.
        const MAP_ANONYMOUS = 1 << 5;
        /// Don't check for reservations.
        const MAP_NORESERVE = 1 << 14;
        /// Allocation is for a stack.
        const MAP_STACK = 0x20000;
    }
}

#[register_trap_handler(SYSCALL)]
fn handle_syscall(tf: &TrapFrame, syscall_num: usize) -> isize {
    ax_println!("handle_syscall [{}] ...", syscall_num);
    let ret = match syscall_num {
         SYS_IOCTL => sys_ioctl(tf.arg0() as _, tf.arg1() as _, tf.arg2() as _) as _,
        SYS_SET_TID_ADDRESS => sys_set_tid_address(tf.arg0() as _),
        SYS_OPENAT => sys_openat(tf.arg0() as _, tf.arg1() as _, tf.arg2() as _, tf.arg3() as _),
        SYS_CLOSE => sys_close(tf.arg0() as _),
        SYS_READ => sys_read(tf.arg0() as _, tf.arg1() as _, tf.arg2() as _),
        SYS_WRITE => sys_write(tf.arg0() as _, tf.arg1() as _, tf.arg2() as _),
        SYS_WRITEV => sys_writev(tf.arg0() as _, tf.arg1() as _, tf.arg2() as _),
        SYS_EXIT_GROUP => {
            ax_println!("[SYS_EXIT_GROUP]: system is exiting ..");
            axtask::exit(tf.arg0() as _)
        },
        SYS_EXIT => {
            ax_println!("[SYS_EXIT]: system is exiting ..");
            axtask::exit(tf.arg0() as _)
        },
        SYS_MMAP => sys_mmap(
            tf.arg0() as _,
            tf.arg1() as _,
            tf.arg2() as _,
            tf.arg3() as _,
            tf.arg4() as _,
            tf.arg5() as _,
        ),
        _ => {
            ax_println!("Unimplemented syscall: {}", syscall_num);
            -LinuxError::ENOSYS.code() as _
        }
    };
    ret
}

#[allow(unused_variables)]
fn sys_mmap(
    addr: *mut usize,
    length: usize,
    prot: i32,
    flags: i32,
    fd: i32,
    _offset: isize,
) -> isize {
    // ****** START xisanlou add at  25122606 No.2
    // 计算所需要分配的内存的大小
    let mem_size = align_up_4k(length);

    // 检查"_offset"与内存页大小4K对齐及大于0
    if _offset < 0 {
        ax_println!("Argument _offset={} is not legitimate. ", _offset);
        return -1;
    }
    if !is_aligned_4k(_offset as usize) {
        ax_println!("Argument _offset={} is not aligned 4k ", _offset);
        return -1;
    }

    // 检查句柄fd的合法性
    if fd < 0 && fd != -1 {
        ax_println!("Argument fd={} is not legitimate.", fd);
        return -1;
    }

    // 转换prot到MappingFlags
    let mapping_flags: MappingFlags;
    if let Some(mprot) = MmapProt::from_bits(prot) {
        mapping_flags = MappingFlags::from(mprot);
    } else {
        ax_println!("Argument prot={} is not legitimate.", prot);
        return -1;
    }

    // 转换参数flags到MmapFlags
    let mmap_flags: MmapFlags;
    if let Some(parse_m_flags) = MmapFlags::from_bits(flags) {
        mmap_flags = parse_m_flags;
    } else {
        ax_println!("Argument flags={} is not legitimate.", flags);
        return -1;
    }

    // 获得当前任务
    let curr = current();

    // 根据参数addr是否为Null来确定内存的起始地址
    let start_addr: VirtAddr;
    if addr.is_null() {
        // 地址为空，自动分配起始地址
        // 获取用户地址空间
        let aspace = curr.task_ext().aspace.lock();
        let range_start = aspace.base();
        let range_end = aspace.end();
        // 在用户地址空间中找到一个可用VirtAddr
        if let Some(virt_addr) = aspace.find_free_area(
                range_start, 
                mem_size, 
                VirtAddrRange::new(range_start, range_end)
            ) 
        {
            // 找到满足要求的内存块
            start_addr = virt_addr;
        } else {
            // 未找到符合要求的内存空间
            ax_println!("Can not find a map area in user address space for sys_mmap.");
            return -1;
        }
    } else {
        // 使用参数传入的地址作为映射起始地址
        
        start_addr = VirtAddr::from_mut_ptr_of(addr);
        // 检查addr是否4k对齐
        if !is_aligned_4k(start_addr.as_usize()) {
            ax_println!("Argument addr={} is not aligned 4k ", start_addr.as_usize());
            return -1;
        }
    }

    // 为sys_mmap分配内存并进行映射
    let mut uspace = curr.task_ext().aspace.lock();
    uspace.map_alloc(start_addr, mem_size, mapping_flags, true).unwrap();

    // 如果fd为-1, 则直接返回起始地址
    if fd == -1 {
        return start_addr.as_usize() as isize;
    }

    // 加载文件到缓冲区
    // 定义缓冲区为Vec
    let length_offset = length + _offset as usize;
    let mut buf: Vec<u8> = Vec::with_capacity(length_offset);
    let buf_ptr = buf.as_mut_ptr();
    // 读取fd内容到缓冲区
    let dst = unsafe { core::slice::from_raw_parts_mut(buf_ptr as *mut u8, length_offset)};
    let result = api::get_file_like(fd as c_int); 
    match result {
        Ok(file) => {
            // 打开文件成功,读取文件内容到缓冲区
            match file.read(dst) {
                Ok(_) => {
                // 读取文件成功
                ax_println!("Read file complete: fd={}.", fd);
                },
                Err(err) => {
                    // 读取文件内容失败
                    ax_println!("Read file Error: fd={} ERROR: {:?}", fd, err);
                    return -1;
                }
            }
        },
        Err(e) => {
            // 打开文件失败
            ax_println!("Open file Error: {:?} fd={}.", e, fd);
            return -1;
        }   
    }

    // 获得start_addr的物理地址
    let (paddr, _, _) = uspace
        .page_table()
        .query(start_addr)
        .unwrap_or_else(|_| panic!("Mapping failed for segment: {:#x}", start_addr));
    ax_println!("paddr: {:#x}", paddr);

    // 复制文件内容到目的内存地址区间
    let ptr2 = buf.as_ptr() as *const u8;
    unsafe {
        core::ptr::copy_nonoverlapping(
            ptr2.offset(_offset),
            phys_to_virt(paddr).as_mut_ptr(),
            length,
        );
    }

    // 返回起始的虚拟内存地址
    start_addr.as_usize() as isize
    // ****** END   xisanlou add at 25122606 No.2
}

fn sys_openat(dfd: c_int, fname: *const c_char, flags: c_int, mode: api::ctypes::mode_t) -> isize {
    assert_eq!(dfd, AT_FDCWD);
    api::sys_open(fname, flags, mode) as isize
}

fn sys_close(fd: i32) -> isize {
    api::sys_close(fd) as isize
}

fn sys_read(fd: i32, buf: *mut c_void, count: usize) -> isize {
    api::sys_read(fd, buf, count)
}

fn sys_write(fd: i32, buf: *const c_void, count: usize) -> isize {
    api::sys_write(fd, buf, count)
}

fn sys_writev(fd: i32, iov: *const api::ctypes::iovec, iocnt: i32) -> isize {
    unsafe { api::sys_writev(fd, iov, iocnt) }
}

fn sys_set_tid_address(tid_ptd: *const i32) -> isize {
    let curr = current();
    curr.task_ext().set_clear_child_tid(tid_ptd as _);
    curr.id().as_u64() as isize
}

fn sys_ioctl(_fd: i32, _op: usize, _argp: *mut c_void) -> i32 {
    ax_println!("Ignore SYS_IOCTL");
    0
}
