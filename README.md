# GrantsTrace — 开源发布活动认证与资助资格索引器

CS5298 三人项目的第一版开发骨架。当前只完成公共契约、状态声明、构造器、共享状态查询和资格判断的纯函数；还不是完整应用。

2026-11-19 骨架验收：C 在 Ubuntu/WSL2 中完成 cargo check、shaker build Cargo.toml、shaker test Cargo.toml，三项退出码均为 0；单元测试实际执行 5 项，5 passed、0 failed。业务功能仍按下表标记。

## 当前能做什么

| 内容 | 状态 |
| --- | --- |
| 公共类型、常量、统一 JSON 响应与错误码 | 已编写，见 src/model.rs |
| Network/Instance 状态、releases oracle topic | 已声明，见 src/lib.rs |
| constructor、get_grant_status | 已编写，待 Lyquor 运行验证 |
| is_eligible | 已编写；只判断快照中的业务条件，不验证签名 |
| A/B/C 其余业务函数 | 明确返回 E_NOT_IMPLEMENTED |
| 委员会 validate 钩子 | 始终拒绝，返回 false |
| accept_snapshot | 仅声明 certified group；占位返回错误，不保存数据 |
| GitHub 取数、委员会认证、积分领取 | 尚未实现 |
| 四节点、部署、外部用户测试 | 尚未执行 |
| Cargo.lock | 以 C 的 Ubuntu 构建生成的文件为准，须提交；分发包不包含它 |

不要把“项目可构建”写成“业务已完成”或“认证成功”。

## 三个人在哪里写代码

| 成员 | 普通 Rust 逻辑 | Lyquor 对外入口 |
| --- | --- | --- |
| A | src/observations.rs | src/api/observations.rs |
| B | src/verification.rs | src/api/verification.rs |
| C | src/grants.rs | src/api/grants.rs |

公共文件 src/model.rs、src/lib.rs、Cargo.toml、Cargo.lock 由 C 维护。字段与签名见 [接口契约](docs/interfaces.md)。

api 目录中的文件由 lib.rs 的 include! 放在 crate 根展开；这些文件中不要重复写 use、mod 或 state!。三个普通业务模块使用 crate::model::* 引入共同类型。

## 环境基线

C 的构建环境：Rust/Cargo 1.99.0、shaker 0.8.4+54c5e11、cast 1.7.1。Cargo 中的 lyquid/lyquid-test 固定为 =0.8.4，使用官方仓库 HTTPS；实际解析到 LDK Git commit 前缀 47a5ce17，完整 revision 以 C 提交的 Cargo.lock 为准。构建与单元测试已验收，部署和多节点行为尚未验证。

shaker 的构建提交号没有被当成 LDK Git revision 使用，因为尚未证实它们属于同一仓库。

## 首次构建：在项目根目录执行

```bash
export PATH="$HOME/.shakenup/bin:$HOME/.cargo/bin:$PATH"
rustup component add rustfmt
rustup target add wasm32-unknown-unknown
cargo fmt --check
cargo check
shaker build --help
```

shaker 0.8.4 的本机 help 和实际执行已确认以下调用：

```bash
shaker build Cargo.toml
```

C 的 Ubuntu 已构建成功，产物为 lyquid_tools_target/release/cs5298-grantstrace/lyquid.pack。其他成员仍须在自己的环境执行。出错时保留错误与退出码，不随意更改版本约束。LDK Git 依赖中的模板文件可能打印 {{project-name}} 包名诊断；C 此次 cargo check 退出码仍为 0，该诊断与之前代码的 E0382 错误不同。

成功后保留 Cargo.lock；A/B 之后使用同一份锁文件，进行 cargo check --locked。Cargo.lock 固定实际 Git commit 和依赖版本，不能用另一台电脑的临时检查工程锁文件代替。

## 测试状态

src/model.rs 与 src/grants.rs 包含 5 个单元测试，针对 JSON 契约和资格判断边界。2026-11-19，C 实际执行结果为 5 passed、0 failed，退出码 0。测试输入 fixtures/offline_candidate.json 是人工合成的，不是 GitHub 实时响应，也不是认证快照证据。

运行命令：

```bash
shaker test Cargo.toml
```

测试函数使用 #[lyquid_test::test] 注册到 guest 测试运行器。验收时同时核对实际用例数量、通过/失败数量和退出码，不能把 0 tests 算作通过。本项目 .cargo/config.toml 默认编译 WASM，普通 Windows cargo test 不是完整的 Lyquor 测试。

## Git 协作

三人直接在 main 开发。开始工作前先保持工作区干净，再执行 git pull --ff-only origin main。每次仅提交自己的文件；公共契约由 C 统一维护。推送被拒时先同步和解决冲突，禁止 force push。

本仓库由 C 初始化和首次推送，A/B 接受协作邀请后 clone 同一仓库，不再各自生成模板。第一次推送前检查 git diff --cached，确保没有个人 token、私钥或节点数据。

## 来源与 AI 使用记录

- 工程模板：[Lyquor LDK](https://github.com/lyquor-labs/ldk/tree/main/lyquid-template)
- [状态模型](https://docs.lyquor.dev/docs/ldk/state/)
- [方法与上下文](https://docs.lyquor.dev/docs/ldk/functions/)
- [认证调用机制](https://docs.lyquor.dev/docs/ldk/oracle/)
- 项目业务类型和签名来自三人实施方案第 8 节。
- 本骨架由 AI 辅助生成。成员须审阅、运行验证并记录自己实际完成的修改。AI 没有替代本机部署、外部用户测试或报告中的实测证据。

