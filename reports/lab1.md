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
进入__restore的时候，sp指向的是TrapContext的值
