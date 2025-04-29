//! Memory management implementation
//!
//! SV39 page-based virtual-memory architecture for RV64 systems, and
//! everything about memory management, like frame allocator, page table,
//! map area and memory set, is implemented here.
//!
//! Every task or process has a memory_set to control its virtual memory.

mod address;
mod frame_allocator;
mod heap_allocator;
mod memory_set;
mod page_table;

pub use address::{PhysAddr, PhysPageNum, VirtAddr, VirtPageNum};
use address::{StepByOne, VPNRange};
pub use frame_allocator::{frame_alloc, FrameTracker};
pub use memory_set::remap_test;
pub use memory_set::{kernel_stack_position, MapPermission, MemorySet, KERNEL_SPACE};
pub use page_table::{translated_byte_buffer, PageTableEntry};
pub use page_table::{PTEFlags, PageTable};
use crate::task::current_user_token;

/// initiate heap allocator, frame allocator and kernel space
pub fn init() {
    heap_allocator::init_heap();
    frame_allocator::init_frame_allocator();
    KERNEL_SPACE.exclusive_access().activate();
}
///get the page from a virtual address
pub fn get_page_from_vir(virt_addr: VirtAddr) -> Option<PhysAddr> {
    let offset = virt_addr.page_offset();
    let vpn = virt_addr.floor();
    let ppn = PageTable::from_token(current_user_token()).translate(vpn).map(|pte| pte.ppn());
    if let Some(ppn) = ppn {
        Some(PhysAddr(usize::from(PhysAddr::from(ppn)) + offset))
    } else {
        None
    }
}
///the read function for trace
pub fn trace_read(token: usize, src: usize) -> Option<u8> {
    println!("get into trace_read");
    let pt = PageTable::from_token(token);
    let start_va = VirtAddr::from(src);
    let vpn = start_va.floor();
    let Some(pte) = pt.translate(vpn) else {
        return None;
    };

    if !pte.is_valid() ||!pte.is_for_user() || !pte.readable() || src & !(1 << 38) != src & !(1 << 40){
        return None;
    }
    println!("the page table get!");
    let address = get_page_from_vir(VirtAddr::from(start_va)).unwrap();
    let val = address.0 as *const u8;
    unsafe {Some(*val) }
}

/// the write function for trace
pub fn trace_write<T>(token: usize, data: T, dst: usize) -> bool {
    debug!("trace_write: dst {:x?}", dst);
    let pt = PageTable::from_token(token);
    let start_va = VirtAddr::from(dst);
    let vpn = start_va.floor();

    let Some(pte) = pt.translate(vpn) else {
        return false;
    };

    if !pte.is_valid() ||!pte.is_for_user() || !pte.writable(){
        return false;
    }

    let src_buf_ptr: *const u8 = unsafe { core::mem::transmute(&data) };
    let len = core::mem::size_of::<T>();

    let dst_frames = translated_byte_buffer(token, dst as *const u8, len);

    let mut offset = 0;
    for dst_frame in dst_frames {
        dst_frame.copy_from_slice(unsafe {
            core::slice::from_raw_parts(src_buf_ptr.add(offset), dst_frame.len())
        });
        offset += dst_frame.len();
    }

    true
}