# B 的贡献记录

## 代码职责

2026-11-22 新分支 `feature/b-integration` 补齐 `src/api/verification.rs`：候选查询、管理员委员会配置、委员会状态、单阶段验证钩子与认证提交。为使认证结果可消费，同分支也补齐 C 的配置、轮次、认证接收和领取状态转换；这些改动需由 C 审阅。

`scripts/committee.sh` 已加入 epoch 激活调用与状态轮询；`scripts/certify.sh` 已改用 Instance 的 `cast call` 并轮询最终快照。两份脚本仍需在新部署上完整实测。

## 2026-11-22 四节点实测

- macOS Docker Desktop 启动后 `docker info` 返回 ServerVersion 29.9.2；四个 Lyquor 节点、Anvil 和 registry 均启动，setup-devnet 退出码为 0。
- `shaker deploy` 已在本地 devnet 部署分支版本；初始化状态查询返回空配置、空快照。
- `configure_committee` 将四个实际 Node ID 与阈值 3 写入 staging；对 Lyquid ID 转换得到的地址调用 `__lyquor_oracle_advance_epoch` 与 `__lyquor_oracle_finalize_epoch` 后，节点 1 和节点 2 查询均为 `configured=true, active=true, threshold=3`。
- `configure_grant` 与 `start_round` 交易已由共享状态查询确认，当前轮次为 1。
- 四节点 `refresh_observation()` 均返回 `E_HTTP`：Lyquor Host HTTP 无法向 GitHub API 发出请求；同一 Docker 网络的工具容器 `curl -I https://api.github.com` 返回 HTTP 200。根因尚未定位，因此正例认证、错误提案、两节点批准不足实验未完成。
- 无候选时 `request_certification()` 返回 `E_NOT_READY`；没有产生认证快照。
- 本轮 `cargo check --locked` 成功，`shaker test Cargo.toml` 为 34 passed、0 failed。更新后的委员会和认证脚本通过 `bash -n`，但完整脚本尚未在一份全新部署上跑通。

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
