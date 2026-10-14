# Rust 与 Java：项目语法对照

这份笔记配合二维码识别项目阅读，解释项目代码里最容易和 Java 思维混淆的 Rust 语法。Rust 和 Java 都有静态类型、枚举、泛型和接口式抽象，但 Rust 把所有权、借用和可变性放进类型与编译检查里；不少 Java 常见做法因此会换成不同写法。

<a id="variables"></a>

## 1. `let`、可变性与变量绑定

Rust 用 `let` 创建绑定，默认不可变；需要改绑定时明确写 `mut`：

```rust
let status = String::from("ready");
let mut count = 0;
count += 1;
```

Java 的局部变量通常默认可重新赋值，`final` 才限制重新赋值。Rust 的选择正好相反：先不可变，需要改变时再声明 `mut`。这限制了状态意外变化。

注意：`mut` 限制的是这个绑定能否改指向/修改其拥有的值，不会让所有别名都自动可变；借用规则仍会检查同一数据是否被同时读写。

<a id="struct-impl-methods"></a>

## 2. `struct`、`impl`、方法接收者与可见性

Rust 用 `struct` 定义数据，用独立的 `impl` 块定义相关方法；没有 `class` 关键字，也没有隐式 `this`：

```rust
struct AppModel {
    status: String,
}

impl AppModel {
    fn clear(&mut self) {
        self.status.clear();
    }
}
```

方法第一个参数写作 `self`，显式说明方法如何使用对象：

- `&self`：借用读取对象，类似 Java 方法读取字段，但借用期限由编译器检查。
- `&mut self`：可变借用，方法可以改对象；同一时间不能另有冲突借用。
- `self`：取得对象所有权，调用后原绑定通常不能再使用。

Rust 字段默认私有。`pub(crate)` 表示整个当前 crate 内可见，允许当前 crate 的其他模块访问，但 Rust 可见性沿模块层级声明，不是 Java 的 package-private 规则。

相关代码：`src/model.rs` 的 `AppModel`、`OverlayState` 和 `reset`。

<a id="ownership-borrowing"></a>

## 3. 所有权、借用、`String` 和引用

每个非复制类型的值有一个所有者。把 `String` 赋给另一个变量或传入按值接收的函数，通常会移动所有权：

```rust
let owned = String::from("二维码内容");
let borrowed: &str = &owned;
println!("{borrowed}");
// owned 仍有效，因为这里只借用了它
```

Java 通常复制对象引用，垃圾回收器管理对象生命周期；Rust 不靠 GC，而是通过移动与借用规则在编译期确保数据何时能读、何时能改、何时释放。

- `String` 拥有可变长度的 UTF-8 文本。
- `&str` 是对文本的借用，不拥有文本。
- `content.clone()` 明确复制字符串内容；`Arc::clone(&value)` 通常只增加共享引用计数，不复制底层图像。

先借用能完成的工作时，通常不需要 clone；跨线程或需要独立所有权时再选择合适的所有权方式。

<a id="option"></a>

## 4. `Option<T>`：显式表示“可能没有值”

Rust 不把普通引用默认为可空。可能缺值时用 `Option<T>`：

```rust
let hwnd: Option<isize> = None;
if let Some(handle) = hwnd {
    println!("窗口句柄：{handle}");
}
```

它可类比 Java 的 `Optional<T>`，但 Rust 的 `Option<T>` 是语言中常用的枚举类型；`Some(value)` 和 `None` 必须显式处理。`if let` 适用于只关心一种分支，`match` 适用于逐项处理多种情况。

项目里的窗口句柄、截图、纹理、完成事件都是可选值，所以使用 `Option` 而不是用 `null` 表示缺失。

<a id="enum-match"></a>

## 5. 枚举可带数据，`match` 必须处理完整

Rust 的枚举变体可以带各自的数据：

```rust
enum MainAction {
    BeginCapture,
    CopyResult(usize),
}
```

`CopyResult(usize)` 不只是一个名字，它还携带结果索引。控制器通过 `match` 分派动作：

```rust
match action {
    MainAction::BeginCapture => { /* 开始截图 */ }
    MainAction::CopyResult(index) => { /* 复制第 index 条 */ }
}
```

这可以类比 Java 的 enum 加字段，或密封类的不同子类型；Rust 编译器会检查 `match` 是否覆盖所有变体。项目中的 `MainAction` 是带数据枚举的例子，控制器用 `match` 处理它的每种变体。

<a id="result-errors"></a>

## 6. `Result<T, E>`、错误处理与 `?`

Rust 函数可以在返回类型中声明成功值和错误：`Result<T, E>`。它不是异常；调用方需要处理 `Ok(value)` 或 `Err(error)`：

```rust
fn read_count() -> Result<usize, String> {
    Ok(3)
}

match read_count() {
    Ok(count) => println!("{count}"),
    Err(error) => eprintln!("{error}"),
}
```

`?` 是把错误交给当前函数的简写：若表达式是 `Err(error)`，函数立即以该错误返回；若是 `Ok(value)`，则取出 `value` 继续执行。它适合逐层传播错误，最终由 UI 或调用方决定如何展示。

与 Java 的 `try/catch` 相比，Rust 常把预期内、可恢复的失败放在返回值里，让函数签名直接展示可能的失败。

<a id="return-expressions"></a>

## 7. 函数返回值与表达式

Rust 函数签名用 `->` 声明返回类型。函数最后一个表达式如果没有分号，就是返回值：

```rust
fn is_ready() -> bool {
    true
}
```

加分号后该行成为语句，通常返回单元类型 `()`，类似“没有有用返回值”的 `void`。`if`、`match` 也能作为表达式产生值，这常减少临时变量和显式 `return`。

<a id="closures-move"></a>

## 8. 闭包、借用捕获和 `move`

闭包用 `|参数| 表达式` 表示，可捕获周围变量：

```rust
let prefix = "二维码";
let label = |index| format!("{prefix} {}", index + 1);
```

闭包默认按需要借用外部变量。加 `move` 会把闭包所需的值捕获进闭包自身；在线程中使用 `move || { ... }`，是为了让线程拥有自己要用的数据，避免引用已经结束的栈帧。

`Arc::clone` 常用于把共享状态的一个新引用交给后台线程。这里复制的是共享句柄，而不是整份状态内容。

托盘菜单等长期回调通常要求 `'static`，即闭包不能借用一个可能先于它销毁的局部变量；因此通过 `Arc` 把需要的共享所有权交给回调。`Fn` 是回调可重复调用的 trait 约束，与 Java 函数式接口的使用场景相近。项目的托盘回调用 `move` 保存 egui 上下文和退出标记，见 `src/system_tray.rs`。

<a id="arc-atomicbool"></a>

### 用 `Arc<AtomicBool>` 把托盘事件交给 UI 线程

托盘菜单回调和 `QrController::ui` 属于不同的事件路径。托盘回调不能直接借用 UI 线程正在使用的控制器，所以项目用一个共享布尔值传递“有一次截图请求待处理”：

```rust
// 托盘菜单回调：写入请求，并唤醒 egui。
capture_requested.store(true, Ordering::SeqCst);
ctx.request_repaint();

// 控制器 UI 帧：读取请求并清零，确保不会重复处理。
if self.capture_requested.swap(false, Ordering::SeqCst) {
    self.begin_capture(ctx);
}
```

- `AtomicBool` 提供跨线程安全的布尔读写，不需要像 `Mutex<bool>` 那样加锁。
- `Arc` 让托盘回调和控制器持有同一个原子值；克隆 `Arc` 只复制共享句柄。
- `store(true, ...)` 发布待处理状态；`swap(false, ...)` 一次性取走状态并清零。
- `Ordering::SeqCst` 选择最直观的顺序一致性。它让多线程对原子操作采用统一顺序；更复杂的高性能代码才需要评估较弱的内存顺序。
- `request_repaint()` 只是请求框架运行下一帧，不会在回调里直接执行 UI。控制器在那一帧中消费标记、切回截图页并开始截图。

这种单一布尔标记适合“有/无一项待处理请求”，连续多次点击会合并成一个待处理状态。如果以后需要排队保留每个事件或携带参数，应考虑消息通道（例如 `std::sync::mpsc`），而不是继续扩展布尔标记。

<a id="arc-mutex"></a>

## 9. `Arc<Mutex<T>>` 与线程安全共享

`Arc<T>` 是线程安全的引用计数指针，允许多个所有者共享同一份数据；`Mutex<T>` 让同一时刻只有一个线程取得可变访问：

```rust
let shared = Arc::new(Mutex::new(OverlayState::default()));
let worker_state = Arc::clone(&shared);
```

它可粗略类比 Java 的共享对象配合 `synchronized`/锁，但 Rust 的锁会通过 guard 管理加锁区间，类型系统也会约束哪些数据能跨线程传递。`lock()` 返回 `Result`，因为持锁线程 panic 后锁可能被标记为中毒。

项目用它在主窗口、截图线程和选区窗口之间传递截图与一次性结果。锁内应尽量只做短暂读写，不要执行截图或二维码解码。

<a id="iterators"></a>

## 10. 迭代器、闭包和 `collect`

项目用迭代器把解码结果逐步转换：

```rust
let contents = grids
    .into_iter()
    .filter_map(|grid| grid.decode().ok())
    .collect::<Vec<_>>();
```

- `into_iter()` 消费集合并逐项移出值；若要保留集合，可用 `iter()` 借用元素。
- `filter_map` 对每项运行闭包，`Some(value)` 保留，`None` 跳过。
- `collect()` 把迭代器收集成目标集合。`::<Vec<_>>` 显式指定集合类型；`_` 让编译器从上下文推断元素类型。

与 Java Stream API 类似，链式迭代器用于过滤和转换；区别是 Rust 的借用/所有权决定能否消费原集合或只读访问元素。

<a id="traits-derive"></a>

## 11. `trait`、`impl` 与 `derive`

`trait` 描述一组类型要提供的行为，接近 Java interface；`impl Trait for Type` 为类型实现该行为：

```rust
impl eframe::App for QrController {
    // 实现框架要求的 UI 方法
}
```

`#[derive(Default)]` 是属性语法：请求编译器为满足条件的类型生成 `Default` trait 实现。它不是运行时注解，也不是继承。

<a id="modules-use"></a>

## 12. 模块与 `use`

`mod model;` 声明并加载同级的 `model.rs` 模块；`use crate::model::AppModel;` 把路径导入当前作用域，方便后面直接写 `AppModel`。Java 的 `import` 主要是名称导入；Rust 还需要在模块树中显式声明子模块。

`pub` 控制其他 crate 是否能使用公开项；`pub(crate)` 只在当前 crate 内公开；省略可见性时默认私有。

<a id="conditional-compilation"></a>

<a id="conditional-compilation"></a>

## 13. 条件编译属性

`#[cfg(windows)]` 和 `#[cfg(not(windows))]` 会在编译时选择适合目标平台的实现。这不是运行时 `if`：未满足条件的代码不会编进该目标程序。它可类比按平台构建配置或预处理分支。

## 阅读代码时的路线

1. 从 `src/main.rs` 看 `mod`、`use`、闭包和 `let-else`。
2. 从 `src/model.rs` 看结构体、可见性、`Option`、`Arc<Mutex<T>>` 和 `&mut self`。
3. 从 `src/view.rs` 看带数据的枚举、闭包如何生成用户动作，以及 `Option` 如何表示“本帧没有动作”。
4. 从 `src/controller.rs` 看 `match`、所有权、`move` 线程闭包和 `Result` 分支。
5. 从 `src/services/` 看 `?`、`cfg`、迭代器和 `collect`。
<a id="unsafe-ffi"></a>

## 14. `unsafe` 与原生 API

Rust 默认要求编译器验证内存安全；调用某些原生接口时需要写 `unsafe` 块，表示这部分操作依赖调用者保证额外条件，例如句柄有效、API 使用方式正确。`unsafe` 不会关闭其他 Rust 检查，也不是“随便做任何事”的开关。

这和 Java 调用 JNI 或 native library 有相似之处：安全边界之外的规则由调用者承担。项目里的 Win32/DWM FFI 封装在 `services/screen_capture.rs`。
