# ch3 实验报告

## 编程作业

- 共实现一个辅助功能和三个要求功能如下：
  - `record_syscall`，记录当前任务调用了某个系统调用一次；
  - `trace_request` 为 $0$，读取 `_id` 地址处一个字节的值作为 `isize` 类型返回；
  - `trace_request` 为 $1$，将 `_data` 视作 `u8` 写入 `_id` 地址处；
  - `trace_request` 为 $2$，返回当前任务调用 `_id` 号系统调用的次数。

- 由于本章还没有引入虚拟地址，以上所指 `_id` 均指内存中绝对地址，但题目表述中“当前任务 `_id` 地址处”的写法容易造成歧义。

## 简答作业

- 正确进入 U 态后，程序的特征还应有：使用 S 态特权指令，访问 S 态寄存器后会报错。 请同学们可以自行测试这些内容（运行 三个 bad 测例 (ch2b_bad_*.rs) ）， 描述程序出错行为，同时注意注明你使用的 sbi 及其版本。
  - 使用的 sbi 及其版本如下：
    ```text
    [rustsbi] RustSBI version 0.3.0-alpha.2, adapting to RISC-V SBI v1.0.0
    [rustsbi] Implementation     : RustSBI-QEMU Version 0.2.0-alpha.2
    ...
    [rustsbi] pmp01: 0x00000000..0x80000000 (-wr)
    [rustsbi] pmp02: 0x80000000..0x80200000 (---)
    [rustsbi] pmp03: 0x80200000..0x88000000 (xwr)
    [rustsbi] pmp04: 0x88000000..0x00000000 (-wr)
    ```
  - `ch2b_bad_address.rs` 错误信息如下：
    ```text
    [kernel] PageFault in application, bad addr = 0x0, bad instruction = 0x804003a4, kernel killed it.
    ```
    注意到在这一阶段我们还没有开始对内存保护，U 态和 S 态只实现了指令/寄存器隔离，所以 U 态向 `0x0` 处的写入不会被操作系统禁止；另一方面，sbi 在这一地址处也几乎没有保护，允许读写，这条指令会被真正执行。但是由于 `0x0` 处并没有被分配，是一个空地址，所以实际执行时会导致硬件错误从而退出（当然这里的硬件指的是 qemu 模拟出来的硬件）；事实上，如果对这一指令略做修改，变为
    ```rust
    (0x100000 as *mut u32).write_volatile(0x7777);
    ```
    则确实会被执行并使得 qemu 立刻关机，信息如下：
    ```text
    [rustsbi-panic] hart 0 panicked at 'wrong state 0!', rustsbi-qemu/src/qemu_hsm.rs:78:18
    [rustsbi-panic] system shutdown scheduled due to RustSBI panic
    ```
  - `ch2b_bad_instructions.rs` 错误信息如下：
    ```text
    [kernel] IllegalInstruction in application, kernel killed it.
    ```
    cpu 在完成译码后注意到当前指令为 `sret`，但处于 U 态下，硬件会自动禁止执行并抛出异常 `Exception::IllegalInstruction`。
  - `ch2b_bad_register.rs` 错误信息如下：
    ```text
    [kernel] IllegalInstruction in application, kernel killed it.
    ```
    这句指令中出现了两次 `sstatus`，容易令人产生困惑，实际上第一个 `sstatus` 是指内存中的 `sstatus` 寄存器，第二个 `sstatus` 是指 rust 程序中声明的 `sstatus`，整句话的意思连起来是：编译器任意选取一个寄存器 `out(reg)`，先将内存中寄存器 `sstatus` 的值读进 `out(reg)` 里，再将 `out(reg)` 里的值读进 rust 变量 `sstatus` 中（当然，编译器选取的 `out(reg)` 和为变量 `sstatus` 分配的地址可能是同一个）。
    类似上面，cpu 在完成译码后注意到需要读取 `sstatus`，但处于 U 态下，硬件会自动禁止执行并抛出异常 `Exception::IllegalInstruction`。

- 深入理解 trap.S 中两个函数 `__alltraps` 和 `__restore` 的作用，并回答如下问题:
  - L40：刚进入 `__restore` 时，`sp` 代表了什么值。请指出 `__restore` 的两种使用情景。
    此时 `sp` 指向内核栈的栈顶，`__restore` 在初始化完成后进入第一个应用、或者在完成 `trap` 处理后返回 U 态时会被使用。
  - L43-L48：这几行汇编代码特殊处理了哪些寄存器？这些寄存器的的值对于进入用户态有何意义？请分别解释。
    ```asm
    ld t0, 32*8(sp)
    ld t1, 33*8(sp)
    ld t2, 2*8(sp)
    csrw sstatus, t0
    csrw sepc, t1
    csrw sscratch, t2
    ```
    这几行汇编代码从内核栈上恢复了 `sstatus, sepc, sscratch` 三个寄存器的值，`sstatus` 帮助 cpu 回到正确的特权级（U 态），`sepc` 告诉 cpu 处理完 trap 后将要执行的下一条指令，`sscratch` 随后会在第 60 行与 `sp` 交换，从而让 `sp` 指向用户栈的栈顶。
  - L50-L56：为何跳过了 `x2` 和 `x4`？
    ```asm
    ld x1, 1*8(sp)
    ld x3, 3*8(sp)
    .set n, 5
    .rept 27
       LOAD_GP %n
       .set n, n+1
    .endr
    ```
    `tp(x4)` 寄存器几乎不会被使用，我们在 `__alltraps` 中也并没有保存它，所以也不需要恢复；`sp(x4)` 会在第 60 行单独被恢复。
  - L60：该指令之后，`sp` 和 `sscratch` 中的值分别有什么意义？
    ```asm
    csrrw sp, sscratch, sp
    ```
    `sp` 指向用户栈栈顶，`sscratch` 指向内核栈栈顶。
  - `__restore`：中发生状态切换在哪一条指令？为何该指令执行之后会进入用户态？
    发生在最后一行 `sret`，由硬件根据 `sstatus` 中的信息自动完成。
  - L13：该指令之后，sp 和 sscratch 中的值分别有什么意义？
    ```
    csrrw sp, sscratch, sp
    ```
    `sp` 指向内核栈栈顶，`sscratch` 指向用户栈栈顶。
  - 从 U 态进入 S 态是哪一条指令发生的？
    trap 由硬件发生，cpu 在执行指令时会检查是否发生 trap，如果发生会由硬件自动保存 `sstatus, sepc` 等信息，然后自动从 U 态进入 S 态（有时也会进入 M 态），然后跳转到 `stvec` 指向的地址。所以，事实上从 `__alltraps` 的第一行代码就已经在 S 态了。