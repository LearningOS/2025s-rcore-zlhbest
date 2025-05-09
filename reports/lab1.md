# lab1 实验总结报告
## 实现功能简述
完成`sys_trace`函数的全部功能，并通过了测试用例。

✅ `trace_request`=0,读取指针数据

✅ `trace_request`=1,写入指针数据

✅ `trace_request`=2,查询当前任务

### 实现逻辑
使用rust的`match`关键字对传入的`trace_request`参数的值进行区分，区分出0、1、2。 当`trace_request`是0或者1的时候逻辑非常简单，
* `trace_request`是0时使用`as`关键字转换为`*const u8`类型的指针，再通过解引用就可以实现读取指针的值。
* `trace_request`是1时同样使用`as`关键字转换为`*mut u8`类型指针，并通过`write_volatile`函数将指针的值改为data
* 获取当前任务调用系统函数的次数功能
    * 在`TaskControlBlock`结构体中新增`call_count`数组字段，用于记录系统调用被调用了多少次
    * 在`task`的`mod`中增加`current_task_call_add`函数，入参为系统调用编号，通过获取到当然运行的task,获取`call_count`数组让对应系统调用编号的下标值+1
    * 在`task`的`mod`中增加`current_task_call_count`函数，入参同样为系统调用编号，获取当前运行的任务,得到对应系统调用编号的调用次数
    * `current_task_call_add`函数可以写在`syscall`函数中，也可以写在`trap`的`trap_handler`函数中。

通过实现这三个功能，完成`sys_trace`函数。

## 问答题 - 简答作业

### 问题1
正确进入 U 态后，程序的特征还应有：使用 S 态特权指令，访问 S 态寄存器后会报错。 
```
[kernel] PageFault in application, bad addr = 0x0, bad instruction = 0x804003a4, kernel killed it.
[kernel] IllegalInstruction in application, kernel killed it.
[kernel] IllegalInstruction in application, kernel killed it.
```
* 第一个程序`bad_address`,程序访问非法地址，报页表错误，异常退出
* 第二个程序`bad_instructions`,程序访问只有在S态才能后使用的非法指令`sret`
* 第三个程序`bad_register`，程序访问了只有在S态下才能访问的寄存器，`sstatus`。

参考: https://rcore-os.cn/rCore-Tutorial-Book-v3/chapter2/1rv-privilege.html

### 问题2
深入理解 trap.S 中两个函数 __alltraps 和 __restore 的作用
#### L40：刚进入 __restore 时，sp 代表了什么值。请指出 __restore 的两种使用情景。
在ch2中 __restore 代码中有一句为`mv sp a0`, 其中a0就是调用`__restore`传入的trapContext的值。
在ch3中 __restore 代码去掉了sp的赋值语句，但是通过代码查看，在__switch中`ld sp, 8(a1)`将传入的taskContext中的TrapContext的值传给了sp,
__switch的代码中`ret`返回到了`__restore`中。所以刚进入`__restore`时，sp代表了刚刚切换完的任务的TrapContext。

#### L43-L48：这几行汇编代码特殊处理了哪些寄存器？这些寄存器的的值对于进入用户态有何意义？请分别解释。
首先在第一题中明确了SP是TrapContext的值, 结合TrapContext结构体的布局可以得出。
```rust
pub struct TrapContext {
    /// General-Purpose Register x0-31
    pub x: [usize; 32],
    /// Supervisor Status Register
    pub sstatus: Sstatus,
    /// Supervisor Exception Program Counter
    pub sepc: usize,
}
```
```
ld t0, 32*8(sp)   # t0 存储的是sstatus寄存器的值， 
ld t1, 33*8(sp)   # t1 存储的是sepc寄存器的值
ld t2, 2*8(sp)    # t2 存储的是sscratch寄存器的值，因为在__alltraps函数中，该寄存器的值被存入了2号寄存器中
csrw sstatus, t0   # csrw指令的意思是将t0的值赋值给sstatus寄存器。
csrw sepc, t1      # 将t1赋值给sepc
csrw sscratch, t2  # 将t2中的值赋值给sscratch
```
sstatus 的 SPP 字段会被修改为 CPU 当前的特权级（U/S）。
sepc 会被修改为 Trap 处理完成后默认会执行的下一条指令的地址。
sscratch则是存放了离开内核态时候的内核栈指针

#### L50-L56：为何跳过了 x2 和 x4？
```
  ld x1, 1*8(sp)
    ld x3, 3*8(sp)
    .set n, 5
    .rept 27
        LOAD_GP %n
        .set n, n+1
    .endr
```
* x2留给了sscratch寄存器，用于存储sscratch寄存器的值。
* x4在`__alltraps`就没有用到，也是从x5开始的。 所以不需要恢复x4的值

#### L60：该指令之后，sp 和 sscratch 中的值分别有什么意义？
```
csrrw sp, sscratch, sp
```
在刚进入`__restore`函数的时候，sp代表的是内核栈，sscratch代表的是用户栈。在经过一次转换后，sp切换到了用户栈, sscratch保存了内核栈的值

#### __restore：中发生状态切换在哪一条指令？为何该指令执行之后会进入用户态？
是在`sret`指令之后实现的，sret指令会完成以下两种功能:
* CPU 会将当前的特权级按照 sstatus 的 SPP 字段设置为 U 或者 S ；
* CPU 会跳转到 sepc 寄存器指向的那条指令，然后继续执行。

#### L13：该指令之后，sp 和 sscratch 中的值分别有什么意义？
```
csrrw sp, sscratch, sp
```
该命令其实是与__restore中的命令有相反的含义。sp指针保存着用户栈的信息，sscratch保存着内核栈的信息，经过交换以后sp变到了内核栈，而sscratch保存了用户栈信息

#### 从 U 态进入 S 态是哪一条指令发生的？
在U态进行系统调用`ecall`,会触发Trap然后进入`TrapHandler`。
```rust
pub fn syscall(id: usize, args: [usize; 3]) -> isize {
    let mut ret: isize;
    unsafe {
        core::arch::asm!(
            "ecall",
            inlateout("x10") args[0] => ret,
            in("x11") args[1],
            in("x12") args[2],
            in("x17") id
        );
    }
    ret
}
```

## 荣誉准则
1. 在完成本次实验的过程（含此前学习的过程）中，我曾分别与 **以下各位** 就（与本次实验相关的）以下方面做过交流，还在代码中对应的位置以注释形式记录了具体的交流对象及内容：

    *无*

2. 此外，我也参考了 **以下资料** ，还在代码中对应的位置以注释形式记录了具体的参考来源及内容：

    *[实现特权级切换](https://rcore-os.cn/rCore-Tutorial-Book-v3/chapter2/4trap-handling.html)*

3. 我独立完成了本次实验除以上方面之外的所有工作，包括代码与文档。
我清楚地知道，从以上方面获得的信息在一定程度上降低了实验难度，可能会影响起评分。

4. 我从未使用过他人的代码，不管是原封不动地复制，还是经过了某些等价转换。
我未曾也不会向他人（含此后各届同学）复制或公开我的实验代码，我有义务妥善保管好它们。
我提交至本实验的评测系统的代码，均无意于破坏或妨碍任何计算机系统的正常运转。
我清楚地知道，以上情况均为本课程纪律所禁止，若违反，对应的实验成绩将按“-100”分计。