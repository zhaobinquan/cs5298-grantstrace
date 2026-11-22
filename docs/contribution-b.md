# B 的贡献记录

## 代码职责

2026-11-22 新分支 `feature/b-integration` 补齐 `src/api/verification.rs`：候选查询、管理员委员会配置、委员会状态、单阶段验证钩子与认证提交。为使认证结果可消费，同分支也补齐 C 的配置、轮次、认证接收和领取状态转换；这些改动需由 C 审阅。

`scripts/committee.sh` 仅完成委员会 staging；epoch 激活仍须按实际部署的 LDK 命令执行并核验。`scripts/certify.sh` 为调用参考，需在四节点 devnet 上核对 ABI 与入口类别。

- 在 `src/verification.rs` 实现候选结果的构建、验证、JSON 编解码和本节点观察结果比对。
- 候选构建会筛选固定时间窗口、按 Release ID 规范排序、去除完全相同的重复记录，并拒绝内容冲突的重复 ID。
- 候选验证会核对 schema、轮次、完整配置、窗口范围、严格 ID 顺序、稳定版本计数和 16 KiB JSON 上限。
- 本节点验证会要求当前观察结果已 ready、属于当前轮次，并与待认证候选完全相同。

## 测试证据

2026-11-19，在 macOS 上使用 `shaker 0.8.4+54c5e11` 执行：

```bash
export PATH="$HOME/.shakenup/bin:$HOME/.cargo/bin:$PATH"
cargo fmt --check
cargo check --locked
shaker test Cargo.toml
```

`shaker test Cargo.toml` 实际执行 10 项测试，结果为 10 passed、0 failed。其中新增 5 项候选构建、重复冲突、计数不一致、紧凑 JSON 往返和本地候选不一致测试。

## AI 使用

AI 协助根据 `docs/interfaces.md` 的固定契约实现纯 Rust 候选校验逻辑并生成测试。成员应继续审阅代码，并在后续委员会/认证集成时记录真实 devnet 实验结果。
