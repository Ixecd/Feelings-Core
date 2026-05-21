# Feelings-OS 开发规范

> 性质：强制约定，不是建议
> 范围：Feelings-OS 仓库所有 Rust/C 代码、文档、提交

---

## 一、双层命名

```
Rust 层         snake_case——fn / let / mod / filename
                CamelCase——struct / enum / trait
C 驱动层        snake_case——函数和变量
                SCREAMING_SNAKE——硬件寄存器地址宏
```

不缩写。`scheduler` 不是 `sched`，`memory_pool` 不是 `mp`。

---

## 二、Rust 层——零 unsafe

所有 Rust 代码不包含 unsafe 块。唯一的例外：FFI 边界——`extern "C" fn` 声明调用 C 驱动函数。FFI 声明集中在 `src/ffi.rs`——是仓库里唯一允许 unsafe 的文件。

---

## 三、C 驱动层——volatile 寄存器

```c
// ✅ volatile 指针映射 FPGA 寄存器
#define EAR_STIM_REG    (*(volatile uint32_t *)0x40010000)
EAR_STIM_REG = 0x19;  // 设置刺激参数

// ❌ 普通指针——编译器可能优化掉
uint32_t *reg = (uint32_t *)0x40010000;
*reg = 0x19;
```

---

## 四、守护进程命名

```
mempoold     ← 'd' 后缀 = daemon = 守护进程
schedulerd
busd
cached
timerd
logd

六个进程，统一 `d` 后缀。进程名就是它的唯一职责。
```

---

## 五、提交格式

```
feat: xxx      — 新功能
fix: xxx       — 修 bug
docs: xxx      — 文档
refactor: xxx  — 重构
chore: xxx     — 杂项
```

---

## 六、禁止事项

```
❌ unsafe Rust——除非在 src/ffi.rs
❌ malloc / free——全部走 mempoold
❌ std::fs / std::net / std::thread
❌ 标准库 println!——走 tracing + logd
❌ 任何 Linux 系统调用
```
