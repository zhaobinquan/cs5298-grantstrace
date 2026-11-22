# 环境与验收记录

## 用户已提供的终端信息

- Ubuntu 26.04.1 LTS，x86_64，WSL2。
- Docker Engine 29.9.2，Docker Desktop 4.97.0。
- Docker Compose 5.6.1，Ubuntu 内 Git 2.53.0。
- rustc/cargo 1.99.0。
- shaker 0.8.4+54c5e11。
- cast 1.7.1。
- cargo-generate 0.26.0。
- 项目由本机 LDK 的 lyquid-template 生成。

这些信息来自终端输出，不能据此推导四节点已启动或项目已运行。

## 骨架准备环境的检查

- 当前代码由 AI 按方案与公开 LDK 文档编写。
- Windows 侧 rustfmt +stable --edition 2024 已执行格式化。
- 独立的离线原生检查工程尝试编译公共类型，但 Windows 应用控制策略阻止 proc-macro2/serde 构建脚本执行（os error 4551）；测试没有运行。
- 上述检查工程不属于团队项目，不随代码包发布。
- 用户 WSL 中 cargo check 和 Shaker build 已确认退出码 0；guest tests 修正标注后实际执行 5 项，全部通过，退出码 0。部署和 ABI 运行检查未执行。
- 跨 include! 的宏组织已经通过 C 的 cargo check 与 Shaker 构建，无需迁入 lib.rs。
- Cargo.lock：本包不包含，首次成功解析和构建后由 C 生成并提交。

## 依赖约定

保留用户模板的 WASM 默认 target、build.rs 导出及 cdylib/rlib。
将直接的 LDK Git URL 从 SSH 改为官方 HTTPS；固定 crate 版本为 =0.8.4。
普通 serde/time 依赖最终由 Cargo.lock 固定，A/B 不自行 cargo update。

## 首次验收后填写

2026-11-19 用户实际 cargo check 已解析到 lyquid 0.8.4（LDK Git commit 前缀 47a5ce17）。
首次检查在 configure_grant、configure_committee、accept_snapshot 报 E0382：占位元组移动了宏后续使用的 ctx。
修订版将这三处及同类 validate 占位中的 ctx 改为 &ctx，借用上下文而不转移所有权。
修复后的 cargo check 和 Shaker 构建均由用户截图确认退出码 0。构建产物为 lyquid_tools_target/release/cs5298-grantstrace/lyquid.pack。
首次 shaker test Cargo.toml 退出码 0，但实际为 running 0 tests。改用原始官方模板已提供的 #[lyquid_test::test] 注册五个测试后，用户截图确认实际运行 5 项，5 passed、0 failed，退出码 0，用例执行耗时 1.78 秒。

| 项目 | 实际命令 | 结果/日志 |
| --- | --- | --- |
| Rust 格式整理 | cargo fmt | 用户执行后继续完成构建/测试；格式静态检查也已在生成环境通过 |
| WASM 类型检查 | cargo check | 用户确认退出码 0，2026-11-19 |
| Shaker 构建 | shaker build Cargo.toml | 用户确认 Build success、退出码 0，2026-11-19 |
| Guest 单元测试 | shaker test Cargo.toml | 修正注册后实际执行 5 项，5 passed、0 failed，退出码 0，2026-11-19 |
| 默认共享状态查询 | 部署后核对生成 ABI | 待填写 |



## 2026-11-22 C 本机更新

以上“Cargo.lock 不包含”“部署未执行”是旧骨架阶段的历史记录。当前仓库已包含锁文件，本次未修改依赖版本。Docker Desktop 的 Ubuntu WSL integration 启用后，已用独立 Compose project `grantstrace-c-devnet` 启动四节点（节点 0.8.4+54c5e11）并实际部署、认证、领取。WSL 中 Docker Engine 为 29.9.2；Rust/Cargo 1.99.0、Shaker 0.8.4+54c5e11、Cast 1.7.1。

本地 registry 请求必须绕开代理；本次使用 `NO_PROXY=127.0.0.1,localhost` 和小写对应变量，没有修改系统代理。测试 node1 通过 `/tmp/grantstrace-c-logging.yaml` 启用 `LYQUOR_LOG=debug` 取得拒绝错误码；重建该测试容器时保留数据卷。GitHub core 配额曾耗尽并中断验收，恢复后已在当前版本分段完成合格、不合格及全新 e2e 部署，详情见 contribution-c.md。
