//! Types related to task management
use super::TaskContext;
use crate::config::TRAP_CONTEXT_BASE;
use crate::mm::{
    KERNEL_SPACE, MapPermission, MemorySet, PhysPageNum, VirtAddr, kernel_stack_position,
};
use crate::trap::{trap_handler, TrapContext};

const SYSCALL_WRITE: usize = 64;
/// exit syscall
const SYSCALL_EXIT: usize = 93;
/// yield syscall
const SYSCALL_YIELD: usize = 124;
/// gettime syscall
const SYSCALL_GET_TIME: usize = 169;
/// sbrk syscall
const SYSCALL_SBRK: usize = 214;
/// munmap syscall
const SYSCALL_MUNMAP: usize = 215;
/// mmap syscall
const SYSCALL_MMAP: usize = 222;
/// trace syscall
const SYSCALL_TRACE: usize = 410;

/// The task control block (TCB) of a task.
pub struct TaskControlBlock {
    /// Save task context
    pub task_cx: TaskContext,

    /// Maintain the execution status of the current process
    pub task_status: TaskStatus,

    /// Application address space
    pub memory_set: MemorySet,

    /// The phys page number of trap context
    pub trap_cx_ppn: PhysPageNum,

    /// The size(top addr) of program which is loaded from elf file
    pub base_size: usize,

    /// Heap bottom
    pub heap_bottom: usize,

    /// Program break
    pub program_brk: usize,

    /// Times of each syscall
    pub syscall_times: [usize; 8],
}

impl TaskControlBlock {
    /// get the trap context
    pub fn get_trap_cx(&self) -> &'static mut TrapContext {
        self.trap_cx_ppn.get_mut()
    }
    /// get the user token
    pub fn get_user_token(&self) -> usize {
        self.memory_set.token()
    }
    /// Based on the elf info in program, build the contents of task in a new address space
    pub fn new(elf_data: &[u8], app_id: usize) -> Self {
        // memory_set with elf program headers/trampoline/trap context/user stack
        let (memory_set, user_sp, entry_point) = MemorySet::from_elf(elf_data);
        let trap_cx_ppn = memory_set
            .translate(VirtAddr::from(TRAP_CONTEXT_BASE).into())
            .unwrap()
            .ppn();
        let task_status = TaskStatus::Ready;
        // map a kernel-stack in kernel space
        let (kernel_stack_bottom, kernel_stack_top) = kernel_stack_position(app_id);
        KERNEL_SPACE.exclusive_access().insert_framed_area(
            kernel_stack_bottom.into(),
            kernel_stack_top.into(),
            MapPermission::R | MapPermission::W,
        );
        let task_control_block = Self {
            task_status,
            task_cx: TaskContext::goto_trap_return(kernel_stack_top),
            memory_set,
            trap_cx_ppn,
            base_size: user_sp,
            heap_bottom: user_sp,
            program_brk: user_sp,
            syscall_times: [0usize; 8],
        };
        // prepare TrapContext in user space
        let trap_cx = task_control_block.get_trap_cx();
        *trap_cx = TrapContext::app_init_context(
            entry_point,
            user_sp,
            KERNEL_SPACE.exclusive_access().token(),
            kernel_stack_top,
            trap_handler as usize,
        );
        task_control_block
    }
    /// change the location of the program break. return None if failed.
    pub fn change_program_brk(&mut self, size: i32) -> Option<usize> {
        let old_break = self.program_brk;
        let new_brk = self.program_brk as isize + size as isize;
        if new_brk < self.heap_bottom as isize {
            return None;
        }
        let result = if size < 0 {
            self.memory_set
                .shrink_to(VirtAddr(self.heap_bottom), VirtAddr(new_brk as usize))
        } else {
            self.memory_set
                .append_to(VirtAddr(self.heap_bottom), VirtAddr(new_brk as usize))
        };
        if result {
            self.program_brk = new_brk as usize;
            Some(old_break)
        } else {
            None
        }
    }
    /// record a system call of id
    pub fn record_syscall(&mut self, syscall_id: usize) {
        match syscall_id {
            SYSCALL_WRITE => self.syscall_times[0] += 1,
            SYSCALL_EXIT => self.syscall_times[1] += 1,
            SYSCALL_YIELD => self.syscall_times[2] += 1,
            SYSCALL_GET_TIME => self.syscall_times[3] += 1,
            SYSCALL_TRACE => self.syscall_times[4] += 1,
            SYSCALL_MMAP => self.syscall_times[5] += 1,
            SYSCALL_MUNMAP => self.syscall_times[6] += 1,
            SYSCALL_SBRK => self.syscall_times[7] += 1,
            _ => panic!("Unsupported syscall_id: {}", syscall_id),
        }
    }
    /// report times of syscall_id
    pub fn report_syscall(&self, syscall_id: usize) -> usize {
        match syscall_id {
            SYSCALL_WRITE => self.syscall_times[0],
            SYSCALL_EXIT => self.syscall_times[1],
            SYSCALL_YIELD => self.syscall_times[2],
            SYSCALL_GET_TIME => self.syscall_times[3],
            SYSCALL_TRACE => self.syscall_times[4],
            SYSCALL_MMAP => self.syscall_times[5],
            SYSCALL_MUNMAP => self.syscall_times[6],
            SYSCALL_SBRK => self.syscall_times[7],
            _ => panic!("Unsupported syscall_id: {}", syscall_id),
        }
    }

    /// Ask for PhysMem for VirtAddr [_start, _start + _len)
    pub fn sys_mmap(&mut self, _start: usize, _len: usize, _port: usize) -> isize {
        if self.memory_set.check_available(VirtAddr(_start),
            VirtAddr(_start + _len)) == false {
            error!("already used VirtAddr in [{:?}, {:?})", _start, _start + _len);
            return -1;
        }
        self.memory_set.mmap(VirtAddr(_start),
            VirtAddr(_start + _len),
            MapPermission::from_bits((_port as u8) << 1).unwrap() | MapPermission::U);
        0
    }

    /// Unmap VirtAddr [start, start + len)
    pub fn sys_munmap(&mut self, _start: usize, _len: usize) -> isize {
        self.memory_set.munmap(VirtAddr(_start).floor(), VirtAddr(_start + _len).ceil())
    }

    /// really map the virtaddr
    pub fn lazy_map(&mut self, virtaddr: usize) -> isize {
        self.memory_set.lazy_map(VirtAddr(virtaddr))
    }
}

#[derive(Copy, Clone, PartialEq)]
/// task status: UnInit, Ready, Running, Exited
pub enum TaskStatus {
    /// uninitialized
    UnInit,
    /// ready to run
    Ready,
    /// running
    Running,
    /// exited
    Exited,
}
