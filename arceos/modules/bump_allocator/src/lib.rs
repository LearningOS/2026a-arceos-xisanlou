#![no_std]

use allocator::{BaseAllocator, ByteAllocator, PageAllocator};
// ****** START xisanlou add at  25102401 No.1
use allocator::{AllocError, AllocResult};
use core::alloc::Layout;
use core::cmp::max;
use core::ptr::NonNull;
use core::mem::size_of;

const MAX_ALIGN_1GB: usize = 0x4000_0000;
// ****** END xisanlou add at  25102401 No.1

/// Early memory allocator
/// Use it before formal bytes-allocator and pages-allocator can work!
/// This is a double-end memory range:
/// - Alloc bytes forward
/// - Alloc pages backward
///
/// [ bytes-used | avail-area | pages-used ]
/// |            | -->    <-- |            |
/// start       b_pos        p_pos       end
///
/// For bytes area, 'count' records number of allocations.
/// When it goes down to ZERO, free bytes-used area.
/// For pages area, it will never be freed!
///
pub struct EarlyAllocator<const PAGE_SIZE: usize> {
    // ****** START xisanlou add at  25102401 No.2
    start: usize,
    end: usize,
    b_pos: usize,
    p_pos: usize,
    count: usize,
    // ****** END xisanlou add at  25102401 No.2
}

impl<const PAGE_SIZE: usize> EarlyAllocator<PAGE_SIZE> {
    pub const fn new() -> Self {
        // ****** START xisanlou add at  25102401 No.3
        Self {
            start: 0,
            end: 0,
            b_pos: 0,
            p_pos: 0,
            count: 0,
        }
        // ****** END xisanlou add at  25102401 No.3
    }
}

impl<const PAGE_SIZE: usize> BaseAllocator for EarlyAllocator<PAGE_SIZE> {
    fn init(&mut self, start: usize, size: usize) {
        // ****** START xisanlou add at  25102401 No.4
        assert!(PAGE_SIZE.is_power_of_two());

        self.start = start;
        self.end = start + size;
        self.b_pos = self.start;
        self.p_pos = self.end;

        // 擦除区间数据
        unsafe {
            core::slice::from_raw_parts_mut(
                self.start as *mut u8,
                self.end - self.start,
            ).fill(0);         
        }
        // ****** END xisanlou add at  25102401 No.4
    }

    fn add_memory(&mut self, start: usize, size: usize) -> AllocResult {
        // ****** START xisanlou add at  25102401 No.5
        unimplemented!()
        // ****** END xisanlou add at  25102401 No.5
    }
}

impl<const PAGE_SIZE: usize> ByteAllocator for EarlyAllocator<PAGE_SIZE> {
    fn alloc(
        &mut self,
        layout: Layout,
    ) -> AllocResult<NonNull<u8>> {
        // ****** START xisanlou add at  25102401 No.6
        // 计算分配的大小
        let size = max(
            layout.size().next_power_of_two(),
            max(layout.align(), size_of::<usize>()),
        );

        // 检查内存是否耗尽
        if self.b_pos + size >= self.p_pos {return Err(AllocError::NoMemory);}

        // 创建返回结果
        let result = NonNull::new(self.b_pos as *mut u8);

        if let Some(result) = result {
            // 内存擦除
            unsafe {
                core::slice::from_raw_parts_mut(
                    self.b_pos as *mut u8,
                    size,
                ).fill(0);         
            }
            // 分配计数自加1
            self.count += 1;
            // b_pos指针右移size
            self.b_pos += size;
            // 返回指针地址的结果
            return Ok(result);
        } else {
            return Err(AllocError::InvalidParam);
        }
        // ****** END xisanlou add at  25102401 No.6
    }

    fn dealloc(&mut self, pos: NonNull<u8>, layout: Layout) {
        // ****** START xisanlou add at  25102401 No.7
        // count 自减1
        self.count -= 1;
        // 如果count降低到0，则清除start到b_pos之间数据，并将b_pos跳回到start
        if self.count == 0 {
            // 擦除已分配字节区间数据
            unsafe {
                core::slice::from_raw_parts_mut(
                    self.start as *mut u8,
                    self.b_pos - self.start,
                )
                .fill(0);
            }
            // b_pos指针回调
            self.b_pos = self.start;
        }
        // ****** END xisanlou add at  25102401 No.7
    }

    fn total_bytes(&self) -> usize {
        // ****** START xisanlou add at  25102401 No.8
        self.used_bytes() + self.available_bytes()
        // ****** END xisanlou add at  25102401 No.8
    }

    fn used_bytes(&self) -> usize {
        // ****** START xisanlou add at  25102401 No.9
        self.b_pos - self.start
        // ****** END xisanlou add at  25102401 No.9
    }

    fn available_bytes(&self) -> usize {
        // ****** START xisanlou add at  25102401 No.10
        self.p_pos - self.b_pos
        // ****** END xisanlou add at  25102401 No.10
    }
}

impl<const PAGE_SIZE: usize> PageAllocator for EarlyAllocator<PAGE_SIZE> {
    const PAGE_SIZE: usize = PAGE_SIZE;

    fn alloc_pages(
        &mut self,
        num_pages: usize,
        align_pow2: usize,
    ) -> AllocResult<usize> {
        // ****** START xisanlou add at  25102401 No.11
        // Check if the alignment is valid.
        if align_pow2 > MAX_ALIGN_1GB || !is_aligned(align_pow2, PAGE_SIZE) {
            return Err(AllocError::InvalidParam);
        }
        let align_pow2_page = align_pow2 / PAGE_SIZE;
        if !align_pow2_page.is_power_of_two() {
            return Err(AllocError::InvalidParam);
        }

        // 页顶对齐页边界
        let pages_top = align_down(self.p_pos, PAGE_SIZE);

        match num_pages.cmp(&1) {
            core::cmp::Ordering::Equal => {
                // 计算新的下界
                let new_p_pos = pages_top - PAGE_SIZE;
                // 检查内存是否耗尽
                if new_p_pos < self.b_pos {
                    return Err(AllocError::NoMemory);
                }
                // 擦除分配区间中的数据
                unsafe {
                    core::slice::from_raw_parts_mut(
                        new_p_pos as *mut u8,
                        self.p_pos - new_p_pos,
                    )
                    .fill(0);
                }
                // 移动指针p_pos
                self.p_pos = new_p_pos;
                // 返回结果
                return Ok(self.p_pos);
            },
            core::cmp::Ordering::Greater => {
                // 计算新的下界
                let new_p_pos = align_down(pages_top - num_pages * PAGE_SIZE, align_pow2);
                // 检查内存是否耗尽
                if new_p_pos < self.b_pos {
                    return Err(AllocError::NoMemory);
                }
                // 擦除分配区间中的数据
                unsafe {
                    core::slice::from_raw_parts_mut(
                        new_p_pos as *mut u8,
                        self.p_pos - new_p_pos,
                    )
                    .fill(0);
                }
                // 移动指针p_pos
                self.p_pos = new_p_pos;
                // 返回结果
                return Ok(self.p_pos);
            },
            _ => return Err(AllocError::InvalidParam),
        };
        // ****** END xisanlou add at  25102401 No.11
    }

    fn dealloc_pages(&mut self, pos: usize, num_pages: usize) {
        // ****** START xisanlou add at  25102401 No.12
        // Undo
        // ****** END xisanlou add at  25102401 No.12
    }

    fn total_pages(&self) -> usize {
        // ****** START xisanlou add at  25102401 No.13
        self.used_pages() + self.available_pages()
        // ****** END xisanlou add at  25102401 No.13
    }

    fn used_pages(&self) -> usize {
        // ****** START xisanlou add at  25102401 No.14
        (self.end - self.p_pos) / PAGE_SIZE
        // ****** END xisanlou add at  25102401 No.14
    }

    fn available_pages(&self) -> usize {
        // ****** START xisanlou add at  25102401 No.15
        (self.p_pos - self.b_pos) / PAGE_SIZE
        // ****** END xisanlou add at  25102401 No.15
    }
}

// ****** START xisanlou add at  25102401 No.16
/// Checks whether the address has the demanded alignment.
///
/// Equivalent to `addr % align == 0`, but the alignment must be a power of two.
#[inline]
const fn is_aligned(base_addr: usize, align: usize) -> bool {
    base_addr & (align - 1) == 0
}

#[inline]
const fn align_down(pos: usize, align: usize) -> usize {
    pos & !(align - 1)
}

#[inline]
const fn align_up(pos: usize, align: usize) -> usize {
    (pos + align - 1) & !(align - 1)
}
// ****** END xisanlou add at  25102401 No.16