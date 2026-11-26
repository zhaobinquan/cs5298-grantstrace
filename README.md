# GrantsTrace — 开源发布活动认证与资助资格索引器

CS5298 三人项目。A 的 GitHub 发布观察、B 的候选验证与四节点认证、C 的资助状态与一次领取均已合入 `main`。本地四节点 devnet 已验证合格、不合格及拒绝路径；各成员的实现与测试记录见 `docs/contribution-*.md`。

`main` 还包含[英文本地演示页面](frontend/README.md)。页面可新建部署、查看四节点观察与认证快照、完成配置和领取，并运行真实四节点验收脚本；本地演示服务使用标准 devnet 测试账户签名。

## English demo video

- [Watch the 66-second demo (English UI, male narration, English captions)](demo/GrantsTrace-demo-English-male-voice.mp4)
- [Download the English subtitle file](demo/GrantsTrace-English.srt)

The video shows an eligible grant with two stable releases and a 3-of-4 certification, followed by an ineligible grant with zero releases and a rejected claim. Credits are local demo values, not real funds.

## 成员代码位置

| 成员 | 普通 Rust 逻辑 | Lyquor 对外入口 |
| --- | --- | --- |
| A(60948290 Xingyi Liu) | src/observations.rs | src/api/observations.rs |
| B(60210283 Qianneng Tang) | src/verification.rs | src/api/verification.rs |
| C(60713255 Binquan Zhao) | src/grants.rs | src/api/grants.rs |

公共文件 src/model.rs、src/lib.rs、Cargo.toml、Cargo.lock 由 C 维护。字段与签名见 [接口契约](docs/interfaces.md)。

api 目录中的文件由 lib.rs 的 include! 放在 crate 根展开；这些文件中不要重复写 use、mod 或 state!。三个普通业务模块使用 crate::model::* 引入共同类型。

## 环境基线

C 的构建环境：Rust/Cargo 1.99.0、shaker 0.8.4+54c5e11、cast 1.7.1。Cargo 中的 lyquid/lyquid-test 固定为 =0.8.4，使用官方仓库 HTTPS；实际解析到 LDK Git commit 前缀 47a5ce17，完整 revision 以 C 提交的 Cargo.lock 为准。已有构建、单元测试和本地四节点实测；已通过与未完成项目分别见 B/C 的贡献记录。

shaker 的构建提交号没有被当成 LDK Git revision 使用，因为尚未证实它们属于同一仓库。

## 首次构建：在项目根目录执行

```bash
export PATH="$HOME/.shakenup/bin:$HOME/.cargo/bin:$PATH"
rustup component add rustfmt
rustup target add wasm32-unknown-unknown
cargo fmt --check
cargo check --locked
shaker build --help
```

shaker 0.8.4 的本机 help 和实际执行已确认以下调用：

```bash
shaker build Cargo.toml
```

C 的 Ubuntu 已构建成功，产物为 lyquid_tools_target/release/cs5298-grantstrace/lyquid.pack。其他成员仍须在自己的环境执行。出错时保留错误与退出码，不随意更改版本约束。LDK Git 依赖中的模板文件可能打印 {{project-name}} 包名诊断；C 此次 cargo check 退出码仍为 0，该诊断与之前代码的 E0382 错误不同。

成功后保留 Cargo.lock；A/B 之后使用同一份锁文件，进行 cargo check --locked。Cargo.lock 固定实际 Git commit 和依赖版本，不能用另一台电脑的临时检查工程锁文件代替。

## 测试状态

在项目根目录执行 `cargo fmt --check`、`cargo check --locked`、`shaker build Cargo.toml`、`shaker test Cargo.toml`。本地 devnet 的委员会、认证和领取实验记录见 [B 的贡献与验证](docs/contribution-b.md)。`fixtures/offline_candidate.json` 是人工样本，不能代替真实 GitHub/API 集成实验。

## 运行与复现

部署、配置、委员会、观察、认证和领取的实际脚本参数与完整流程见 [运行指南](scripts/README.md)。内部新增固定委员会状态，本版本使用新部署；不要直接升级旧状态。CLI 仅操作本地演示积分，无真实资金。

## Git 协作

A、B、C 与英文前端的已审阅集成提交均在 `main`。推送被拒时先同步和解决冲突，禁止 force push。

本仓库由 C 初始化和首次推送，A/B 接受协作邀请后 clone 同一仓库，不再各自生成模板。第一次推送前检查 git diff --cached，确保没有个人 token、私钥或节点数据。

## 来源与 AI 使用记录

- 工程模板：[Lyquor LDK](https://github.com/lyquor-labs/ldk/tree/main/lyquid-template)
- [状态模型](https://docs.lyquor.dev/docs/ldk/state/)
- [方法与上下文](https://docs.lyquor.dev/docs/ldk/functions/)
- [认证调用机制](https://docs.lyquor.dev/docs/ldk/oracle/)
- 项目业务类型和签名来自三人实施方案第 8 节。
- 本项目由 AI 辅助生成。成员须审阅、运行验证并记录自己实际完成的修改。部署与多节点验证的实际结果已单独记录；报告应区分单元测试与 devnet 实验。
