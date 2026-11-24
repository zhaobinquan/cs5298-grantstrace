# C 的贡献与接手记录

## 已完成的骨架工作

C 配置了 WSL2 Ubuntu、Docker Desktop、Rust/Cargo、Shaker 和 Cast，依据官方模板建立共享类型、状态布局、构造器、查询接口和 A/B/C 占位入口，修复 ctx 借用及 guest test 注册问题。2026-11-19 的 5 项骨架测试通过，随后将源码与 Cargo.lock 推送到公开仓库，初始提交为 `e981c92`。

## 2026-11-22 接入与归属

本次以远端 `feature/b-integration` 的 `47eb9cb027aea81648a67eb05da4d771de42142e` 为基础，确认包含当时 main 的 A 提交 `d12e83d` 和更早的 B 候选纯函数提交 `00c5a7f`。本地通过 fetch 后 detached checkout 接入。随后按 C 的明确授权，将 B 的 PR #1 以 merge commit `5ed4fb1` 合并到远端 main；合并提交与 B head 的文件树完全相同。C 的补充修改整理到从该 main 建立的 `feature/c-integration` 分支，供 PR 审阅，不直接合并 main。用户的 `history/` 保留在本地并由 `.gitignore` 忽略。

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

## 实际验证（由 Codex 在 C 的 WSL 执行）

日期：2026-11-22，Asia/Shanghai。环境为 Rust/Cargo 1.99.0、Shaker 0.8.4+54c5e11、Cast 1.7.1、锁定 LDK `47a5ce17`、Docker Engine 29.9.2。以下测试在 C 提交前、基于 `47eb9cb` 的工作树执行；B 的合并提交 `5ed4fb1` 与该基线文件树相同。整理分支时未改变已验证的业务源码。

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

错误候选内容、配置边界、溢出、替换委员会策略和 signer 前缀由 guest 测试验证；没有伪称构造过真实网络恶意签名证书。不同构建的镜像摘要分别记录，不把某次实验归给另一个包。

## 当前版本续跑与剩余人工工作

用户要求保留 A/B 文件原样后，C 撤回对 A 的 `scripts/e2e.sh` 及 B 的 `src/api/verification.rs`、`scripts/committee.sh`、`scripts/certify.sh` 的本地修改；A/B 专属源码和脚本与 `47eb9cb` 完全相同。撤回前的实测保留为历史证据，不归到当前版本。

首轮当前代码部署镜像为 `sha256:edb8502ce3a3ef553c1486923242d268b98e79418cb442b0fd11661a6177fa2f`，四节点 hosting 一致。北京时间 18:06 左右，第一次完整验收进程在合格部署 `0x36c02da8a0983159322a80ffe9f24b1acff8b570` / `Lyquid-xydh5ecomgg2rukd7qbw6gjv7cuf4uiktzbaa` 的第二轮取数遇到 GitHub `E_RATE_LIMIT`，退出 1；此前已通过配置、权限、委员会、两节点 `E_CERT_REJECTED` 和旧候选 `E_NOT_READY`。配额于 18:29:59 恢复后，在该部署上用 `--resume-qualified` 完成三节点认证、一次领取、重复领取、非受益人、冻结和普通交易入账拒绝；最终 `stable_count=2`、3 个已验签 signer、`eligible=true`、`claimed=true`、`credits=100`。

同一次续跑新建门槛 3 的部署 `0x922d6956c99e12dfeb3224dea977d0939758a1fe` / `Lyquid-ffy2ydsoq6nb5m3olukehcayvv4q5usycquaa`，四节点均完成真实 GitHub 观察，3/4 认证生成最终快照；`stable_count=2`、`eligible=false`、`claimed=false`、`credits=0`，受益人领取收到 `E_NOT_ELIGIBLE`，共享状态未变。续跑进程退出 0，打印 `PASS local devnet acceptance suite`。这说明全套场景经限流后的分段续跑完成；不把初次退出 1 写成单次通过。

另以 A 原版 `scripts/e2e.sh` 搭配 C 的 `deploy/configure/c-committee/start-round/c-certify/claim` 适配器运行全新部署 `0x2e2ed0cfd3ad2f1d34481277b3204d807ca2f8c2` / `Lyquid-o535anxf4nby7ash7zldechz4pd6x4gpitzaa`。四个 RPC 在同一 chain、四节点观察、认证和领取均完成，命令退出 0；最终 `round_id=1`、`stable_count=2`、3 个 signer、`eligible=true`、`claimed=true`、`credits=100`。A 的 e2e 没有设置额外 `NEGATIVE_SCRIPT`，所以其日志中的“negative checks pending”只指可选外置检查；上述独立验收脚本已实际完成负例。

为排除分段续跑的编排差异，又完整连续运行一次 `test_devnet.py`，进程退出 0 并打印 `PASS local devnet acceptance suite`。新合格部署 `0x202cce504e04bed6fc0521238ddf04bc9e8e15ab` / `Lyquid-5h2kjsj3cbx2haipw6dzpd7cbe52z3seyfkaa`，新不合格部署 `0x86a2ee8faf9a840f7a2c64ca3d51209f9a02081d` / `Lyquid-tldda6zqdhab2kdysepcqxx73uvm3sxz7wbaa`；两份部署在四节点上镜像摘要均为 `sha256:905abf23dcfdd77fdc8f02d42cb1e22f62365aa5b07a3f56295491732bd91568`。合格场景最终已领取 100 积分；不合格场景保存了快照但 `eligible=false`、`credits=0`，领取被 `E_NOT_ELIGIBLE` 拒绝。不同运行产生不同包摘要，分别记录，不混用。

目前剩余组外用户测试、C 人工代码讲解验收、报告和录屏；它们不是本次代码的未实现功能。真实恶意证书不可由普通公开入口构造，相关策略由 guest 测试和锁定 LDK 源码核验，不声称完成了线上伪签名攻击实验。内部状态布局变化要求新部署，旧部署不能原地升级。C 分支只供审阅，尚未合并 main。

本机原始日志：`/tmp/grantstrace-c-tests.log`、`/tmp/grantstrace-c-build.log`、`/tmp/grantstrace-c-devnet-tests.log`。仅提交经过整理的 Markdown 证据，不提交原始日志、私钥、节点数据或构建产物。

## AI Use Statement

OpenAI Codex 辅助接手审查、最小兼容修复、C 测试与脚本编写、在 C 的 WSL 中运行上述真实命令及整理文档。C 的骨架贡献、A 的观察实现、B 的候选/认证及初版 C 状态机保留原归属；本记录不声称 C 人工编写了 B 的代码。C 仍需逐段阅读、确认采用并能独立解释。测试区分人工 fixture/mock、guest runner 和真实 devnet，限流中断按失败/未完成记录。

后续按照用户要求继续实测，并在 GitHub 限流恢复后完成当前版本的合格、不合格和 fresh e2e 三条路径；以上均保留了实际中断及续跑记录。
