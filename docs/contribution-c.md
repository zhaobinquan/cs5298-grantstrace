# C 的贡献与记录

## 已完成的骨架工作

C 配置了 WSL2 Ubuntu、Docker Desktop、Rust/Cargo、Shaker 和 Cast，依据官方模板建立共享类型、状态布局、构造器、查询接口和 A/B/C 占位入口，修复 ctx 借用及 guest test 注册问题。2026-11-19 的 5 项骨架测试通过，随后将源码与 Cargo.lock 推送到公开仓库，初始提交为 `e981c92`。

## 2026-11-22 接入与归属

B 在该分支已经写出 C 的配置、轮次、快照及领取主体，本次保留并审阅这些实现，不归为 C 从零编写。A 的观察逻辑、候选输入及原有测试保持不变。C 本次完成：

| 文件 | 实际工作 |
| --- | --- |
| `src/grants.rs` | 保留 B 的纯状态机，统一最终配置冻结错误；加入验签 signer 前缀处理；增加配置、轮次、快照、领取、溢出及失败状态保持测试 |
| `src/api/grants.rs` | 保留权限与整体状态写回，强化 certified caller、实际委员会、证书 config hash/epoch、真实 signer 解析与成员检查；只记录 LDK 确实验过的三个 signer |
| `src/lib.rs` | 新增内部固定委员会节点集合，提供实际委员会匹配检查和 LDK staging 替换反例测试 |
| `scripts/deploy.sh`、`configure.sh`、`start-round.sh`、`claim.sh`、`grants_cli.py` | 完成部署、配置、开轮、领取适配器；记录真实部署标识，检查实际 caller 对应账户，轮询完整业务结果，保留 u64 精度，失败不自动重发 |
| `scripts/c-committee.sh`、`c-certify.sh` | C 的独立适配器处理异步 staging/epoch/finalize、A/B 变量兼容及最终候选比对；B 原脚本保留原样 |
| `scripts/test_grants_cli.py`、`test_devnet.py` | 离线脚本回归及真实四节点验收；真实拒绝要求当前 Lyquid 的节点错误码与状态不变同时成立 |
| `.gitignore` | 忽略 Python 缓存目录 |

公共业务 JSON 类型、ABI 签名、依赖与 Cargo.lock 均未改变。C 在首次 `start_round` 从已激活的 3/4 委员会固定 `committee_nodes`，因此要求先激活委员会再开轮。新增状态改变内部持久布局，采用新部署，禁止据此宣称可以无迁移覆盖旧部署。平台自动导出初始化入口仍存在，应用的独立 pin 会拒绝偏离策略的认证；部署 creator 仍可能破坏可用性。详细边界见 `interfaces.md` 末尾。

## 实际验证

日期：2026-11-22，Asia/Shanghai。环境为 Rust/Cargo 1.99.0、Shaker 0.8.4+54c5e11、Cast 1.7.1、锁定 LDK `47a5ce17`、Docker Engine 29.9.2。

| 检查 | 实际结果 |
| --- | --- |
| cargo fmt --check；C 的 api include 文件独立 rustfmt --check | 退出 0 |
| cargo check --locked | 退出 0 |
| shaker build Cargo.toml | 退出 0，产出 lyquid.pack |
| shaker test Cargo.toml | **42 passed，0 failed，0 ignored**；原有 34 项加 C 本次 8 项 |
| python3 -m unittest discover -s scripts -p 'test_*.py' -v | **20 passed**；A 原有 5 项，加 C 的 15 项 |
| bash -n（8 个 shell 脚本）、Python 编译、git diff --check | 通过 |
| 真实 devnet 主流程与权限反例 | 下述已执行场景通过 |
| test_devnet.py 真实验收 | 初次进程因 GitHub `E_RATE_LIMIT` 退出 1；配额恢复后 `--resume-qualified` 进程退出 **0**。随后另用全新部署完整连续运行，进程也退出 **0**，合格、不合格、权限与拒绝场景全部通过 |
| e2e.sh 完整 fresh deployment 路径 | 离线编排回归通过；当前代码另以全新部署真实运行，退出 **0**，完成四节点观察、认证和领取 |

构建仍打印上游 `{{project-name}}` 模板包名诊断和 atomics 警告，最终退出码和测试计数如上；未修改 LDK 缓存来隐藏它们。

独立 Compose project 为 `grantstrace-c-devnet`，未删除旧环境或数据卷。node1 为取得业务错误证据启用了 debug 日志，其他节点保留默认日志级别。首次部署本地 registry 请求因代理返回 502，设置 localhost NO_PROXY 后成功。第一次负例日志采集因默认 info 级别未显示业务错误而超时，启用 debug 后重跑实际用例通过，没有把超时算成通过。

真实 GitHub 输入：`jqlang/jq`，repo_id=5101141，窗口 `[1748736000,1754006400)`，Release ID 222262711、229086848，正式数 2。admin=`0xf39fd6e51aad88f6f4ce6ab8827279cfffb92266`，beneficiary=`0x70997970c51812dc3a010c7d01b50e0d17dc79c8`，均为本地 devnet 账户；奖励仅为内部积分。

- 手动首次验证：部署 `0x0165878a594ca255338adfa4d48449f69242eb8f` / `Lyquid-ahjg4huwvban3bvhxue7rcbaw4mnltwyolhqa`，镜像 `sha256:799a73a75964432837f65ff83eec8e57007a28bb2b8a786dba6f676719f86965`。2 个 ready 节点认证拒绝、3 个 ready 节点形成快照、受益人获得 100 积分均通过。
- 完整验收脚本的合格部署：`0x68b1d87f95878fe05b998f19b66f4baba5de1aed` / `Lyquid-zz2juejwa3xj5cx6jqespjhlzzr7bjchoi7aa`，四节点 hosting 查询确认镜像均为 `sha256:85aa288ac5ef3f0552136ef0bf49f29e1a66ff73c7821862f8c8a7a6e0d9d5fc`。轮次 2 的最终状态：snapshot 存在、3 个已验签 signer、eligible=true、claimed=true、credits=100。
- 未完成的不合格部署：`0x9e545e3c0baab3e08cdfd552c960a1050f373042` / `Lyquid-rkkfz4d3tvxcp4aflxpqu26gxmhyxepvei3qa`，同脚本镜像；门槛 3，轮次 1，委员会已激活。node1 ready；node2 返回 E_RATE_LIMIT 且 candidate=null；未保存快照、未领取。

逐项真实拒绝证据：未配置开轮 `E_NOT_CONFIGURED`；非管理员配置/开轮/委员会设置 `E_UNAUTHORIZED`；无效奖励配置 `E_BAD_CONFIG`；重复配置/委员会设置 `E_ALREADY_CONFIGURED`；无快照领取 `E_NOT_ELIGIBLE`；未激活委员会开轮 `E_COMMITTEE_NOT_READY`；仅两个 ready 节点认证 `E_CERT_REJECTED`；新轮旧候选查询 `E_NOT_READY`；非受益人领取 `E_UNAUTHORIZED`；重复领取 `E_ALREADY_CLAIMED`；最终快照后配置/开轮 `E_FINALIZED`。Network 写入拒绝逐项同时确认节点错误码与共享业务状态未改变。直接普通 `accept_snapshot(string)` 交易无法提交，状态未改变；没有添加测试入账后门。

错误候选内容、配置边界、溢出、替换委员会策略和 signer 前缀由 guest 测试验证；没有伪称构造过真实网络恶意签名证书。

## AI Use Statement

使用AI辅助模块实现，调试排错，测试执行
