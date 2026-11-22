# B 的贡献与验证记录

## 实现范围

`src/verification.rs` 实现候选构建、规范 JSON、完整配置和轮次校验，以及本节点观察结果比对。`src/api/verification.rs` 实现候选查询、管理员委员会配置、委员会状态、单阶段验证钩子和认证提交。候选仅从本节点已就绪的观察缓存取得，验证钩子拒绝参数、目标或本地候选不符的调用。

为跑通认证结果的消费，同分支补齐 C 的配置、轮次、认证接收和领取状态转换。这部分需要 C 审阅。`scripts/committee.sh` 完成 staging、epoch 激活，并逐一确认四节点的委员会状态；`scripts/certify.sh` 提交认证并轮询共享快照。

## 2026-11-22 本地四节点实验

在 macOS Docker Desktop 29.9.2 上启动 Anvil、registry 和四个 Lyquor 节点；`docker info` 可连接 daemon。用 `shaker deploy Cargo.toml -r http://127.0.0.1:8000/lyquids` 部署本分支，脚本将四个实际 Node ID 以阈值 3 激活，四节点 `get_committee_status` 均返回 `configured=true, active=true`。

本机 VPN DNS 把 `api.github.com` 解析到 `198.18.0.44`，Lyquor Host HTTP 请求因此返回 `E_HTTP`。临时 Docker Compose override 将四个节点的 `api.github.com` 映射到当时经 DNS over HTTPS 查询得到的真实 A 记录 `140.82.113.6`，重建节点后 GitHub 请求成功。该地址只是本次实验值，不应固定到项目代码；换环境时需重新确认网络和 DNS。

| 场景 | 实际结果 |
| --- | --- |
| 团队仓库历史空窗口，四节点 ready | 正式发布数 0；认证快照含 3 个实际签名者；`eligible=false` |
| `jqlang/jq`（repo_id 5101141），窗口 `[1748736000,1754006400)`，门槛 2 | 四节点 ready，候选含 Release ID 222262711、229086848，正式数 2；认证快照含 3 个实际签名者，`eligible=true` |
| 仅节点 1、2 ready，节点 3、4 未刷新 | `request_certification` 返回 `E_CERT_REJECTED`，共享 `snapshot=null`；节点 3 刷新后同一部署认证成功，签名者恰为 3 个 |
| 受益人领取 | `claimed=true, credits=100` |
| 重复领取、非受益人领取 | 节点分别记录 `E_ALREADY_CLAIMED`、`E_UNAUTHORIZED`，共享状态仍为 `claimed=true, credits=100` |
| 同数量不同 Release ID 的错误候选 | `verify_observation_rejects_different_records_with_the_same_count` 单元测试返回 `E_CANDIDATE_MISMATCH`；公开认证入口不允许传入外部候选，故未在 devnet 伪造提案 |

Lyquor 的异步合约调用中，`cast send` 交易 receipt 为 `status=0x1` 不等于业务函数成功。错误路径以上述节点错误码及后续共享状态为依据。`request_certification` 的 `submitted=true` 也仅表示已提交；最终结果以 `get_grant_status` 中的快照为准。

## 静态与单元验证

在分支上执行 `cargo fmt --check`、`cargo check --locked`、`shaker build Cargo.toml`、`shaker test Cargo.toml` 和 `bash -n scripts/committee.sh scripts/certify.sh`。单元测试覆盖候选筛选、去重、冲突、规范 JSON、计数和本地观察不一致；实测结果见本次提交前的最终命令输出。

## AI 使用

AI 协助实现、调试和运行以上实验。B 和 C 应审阅各自代码，并按课程要求记录成员贡献。临时 devnet 私钥和 DNS override 不提交到仓库。
