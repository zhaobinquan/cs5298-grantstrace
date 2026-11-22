# GrantsTrace 接口契约 v1

以下复制自小组三人实施方案第 8 节。业务字段、方法签名保持兼容；下列为功能规范。2026-11-22 C 接入 B 集成提交后的实现和实测状态见 contribution-c.md。

## 8. 固定接口与数据契约 v1

**本节是实现规范。三人直接使用，不再分别设计字段、返回值或方法名。** Rust 是静态类型语言，函数参数、返回类型和结构字段需要明确。文中 `String` 是字符串，`u64/u32/u16/u8` 是不同位宽的无符号整数，`bool` 是布尔值，`Vec<T>` 是列表，`Option<T>` 表示可以没有值，`Result<T,E>` 表示成功值或错误。

以下业务接口由本项目决定；LDK 宏、上下文和 ABI 适配仍需在首次项目编译时核验。若课程版本有差异，调整适配层，保持本节的业务契约。本文提供签名与数据规范，不提供函数实现体。

### 8.1 文件、命名和序列化规则

- 所有公共数据类型放在 `src/model.rs`，由 C 按下表一次建立。所有字段对其他模块公开，字段名保持 snake_case。
- `AppResult<T>` 固定为 `Result<T, AppError>`。普通业务函数使用它；LDK 入口使用 `LyquidResult<T>`，由入口负责错误转换。
- 业务结构使用 `Clone`、`Debug`、`PartialEq`、`Eq`，需要 JSON 的结构加 `serde::Serialize`、`serde::Deserialize`。配置与候选反序列化拒绝未知字段；缺失必需字段报错。
- JSON 键与字段同名；数字使用整数，不使用字符串或浮点数；布尔值为 true/false；`Option::None` 序列化为 null。脚本使用 Python/jq，避免在 JavaScript Number 中损失大整数精度。
- 时间全部为 **UTC Unix 秒**。窗口为 `[window_start, window_end)`。请求时钟仅用于诊断与取数前检查，不加入候选一致性比较。
- 地址字符串固定为 `0x` 加 40 个小写十六进制字符，拒绝零地址作为受益账户。C 的入口负责与真实运行时调用者地址比较。
- Node ID 字符串使用 LDK 的标准显示格式；节点列表由实际 devnet 取得。转换为 LDK `NodeID` 的适配代码由 B 负责。
- 首版常量：`SCHEMA_VERSION=1`、`RULE_VERSION=1`、`MAX_PAGES=3`、`PAGE_SIZE=100`、`MAX_WINDOW_RECORDS=32`、`MAX_CANDIDATE_BYTES=16384`、`MAX_PAGE_BYTES=524288`、`HTTP_TIMEOUT_MS=10000`、`CERT_TIMEOUT_MS=30000`。这些是项目上限；平台若有更低限制，启动时应明确报错。

### 8.2 公共数据类型：按表建立 struct

**GrantConfig：一个部署只有一份不可变配置。**

| 字段 | Rust 类型 | 固定含义与约束 |
| --- | --- | --- |
| grant_id | String | 首次演示用 `grant-001`；1–64 个 ASCII 字母、数字、`-` 或 `_` |
| repo_owner | String | GitHub owner，1–100 字符；URL 构造前校验为字母、数字或 `-` |
| repo_name | String | 仓库名，1–100 字符；字母、数字、`-`、`_`、`.`，拒绝单独的 `.` 或 `..` |
| repo_id | u64 | GitHub 数字仓库 ID，大于 0；每次取数重新核对 |
| window_start | u64 | 窗口开始秒，必须小于结束秒 |
| window_end | u64 | 窗口结束秒；A 取数时要求它不晚于本节点当前时间 |
| min_stable_releases | u32 | 演示值 2，允许 1–32；失败资格测试新部署用 3 |
| beneficiary | String | 上述规范化的本地测试账户地址 |
| reward_credits | u64 | 演示值 100，允许 1–1,000,000 |
| rule_version | u16 | 必须为 1 |

**ReleaseRecord：API 解析后、可供计算使用的一条记录。**

| 字段 | Rust 类型 | 含义 |
| --- | --- | --- |
| id | u64 | 大于 0 的 Release ID |
| published_at | u64 | 成功解析后的 UTC Unix 秒 |
| draft | bool | GitHub 字段原值 |
| prerelease | bool | GitHub 字段原值 |

GitHub 原始 DTO 由 A 放在自己的模块：`id: u64`、`published_at: Option<String>`、`draft: bool`、`prerelease: bool`，允许忽略 API 的其他字段。`published_at=null` 且 draft=true 的草稿不进入 ReleaseRecord；其他必要字段缺失、类型不对或时间解析失败均报错。公开 API 通常不提供草稿，本项目不声称观察到未公开内容。

**Candidate：整个内容被认证，字段顺序按本表固定。**

| 字段 | Rust 类型 | 含义 |
| --- | --- | --- |
| schema_version | u16 | 必须为 1 |
| round_id | u64 | 大于 0，等于当前 Network State 轮次 |
| config | GrantConfig | 完整复制固定配置，绑定仓库、窗口、受益人和资助规则 |
| releases | Vec<ReleaseRecord> | 窗口内的全部可解析记录，包括预发布；按 ID 升序、无重复，最多 32 条 |
| stable_count | u32 | 上述列表中 draft=false 且 prerelease=false 的数量 |

**其他公共结构。** 下表字段均必需；只有 Option 类型可为 null。

| 类型名 | 字段与 Rust 类型 | 使用者 |
| --- | --- | --- |
| AppError | `code: String`；`message: String`；`retryable: bool` | 三人共用的业务错误 |
| FetchResult | `releases: Vec<ReleaseRecord>`；`page_count: u8`；`observed_at: u64` | A 返回完整取数结果；releases 此时可含窗口外记录，最多 300 条 |
| ObservationState | `round_id: u64`；`ready: bool`；`candidate: Option<Candidate>`；`error: Option<AppError>`；`page_count: u8`；`observed_at: u64` | 当前节点的本地准备状态 |
| ObservationView | `node_id: String`；`current_round_id: u64`；`observation: ObservationState` | A 的查询/刷新结果 |
| CertifiedSnapshot | `candidate: Candidate`；`signer_node_ids: Vec<String>` | C 保存的最终认证历史；signer 列表来自 LDK 认证上下文 |
| GrantState | `config: Option<GrantConfig>`；`round_id: u64`；`snapshot: Option<CertifiedSnapshot>`；`claimed: bool`；`credits: u64` | C 负责的完整业务共享状态 |
| GrantStatus | `admin: String`；`state: GrantState`；`eligible: bool` | C 对外查询，eligible 从最终快照和门槛计算 |
| CommitteeConfig | `node_ids: Vec<String>` | 初始化输入，恰好四个不同的实际 Node ID；阈值固定为 3 |
| CommitteeView | `configured: bool`；`active: bool`；`threshold: u16`；`node_ids: Vec<String>` | B 查询配置和同步后的状态；未配置时 false/false/0/空列表 |
| CertificationSubmission | `grant_id: String`；`round_id: u64`；`submitted: bool` | B 的成功返回，submitted=true 仅表示已经提交，最终状态另查 |
| ApiResponse<T> | `ok: bool`；`data: Option<T>`；`error: Option<AppError>` | 所有返回 JSON 的业务入口使用的统一外壳 |

`ApiResponse` 的成功组合只能是 `ok=true, data=Some(...), error=None`；失败组合只能是 `ok=false, data=None, error=Some(...)`。`get_candidate` 在尚未 ready 时返回失败，避免成功但没有候选的歧义。

### 8.3 持久状态与状态转换

根文件只声明一次 `state!`，字段固定如下。LDK 的类型直接通过其 prelude 引入。

| 字段名 | 归属与类型 | 初始值/负责方 |
| --- | --- | --- |
| admin | Network，LDK `Address` | 构造器记下真实部署调用者；C |
| grant | Network，`GrantState` | config/snapshot=None，round_id=0，claimed=false，credits=0；C |
| releases | Network，LDK oracle topic | `network oracle releases;`；C 声明，B 实现委员会配置与认证入口 |
| observation | Instance，`ObservationState` | round_id=0，ready=false，candidate/error=None，page_count/observed_at=0；A |

配置成功时 round_id 仍为 0；管理员调用 `start_round` 后变为 1。尚无最终快照时可继续加一，使用 checked_add 防溢出。新轮次使全部旧观察失效，无需 Network 方法修改 Instance State。

A 的查询若发现本地 round_id 不等于当前轮次，对外返回 ready=false、candidate=null 和 `E_ROUND_MISMATCH`。同一轮已经 ready 时，重复刷新返回已有结果，不覆盖正在参与认证的候选。需要重新取数时由管理员开启新轮。

刷新失败时把当前轮本地状态写成 ready=false、candidate=None、error=实际错误；失败页的部分数据不进入候选。Instance 刷新通过正常的 JSON 错误响应返回，避免用会回滚本地修改的异常路径丢失失败状态；运行时写入语义在项目首次集成中核验。

最终快照一旦接受，禁止改配置、开启新轮和接受第二份快照。领取成功把 claimed 设为 true、credits 加一次 reward_credits。错误路径不改变共享状态。正式数量为 0 也可以被认证，此时 eligible=false。

### 8.4 模块内部函数签名

下面是普通 Rust 函数，所有名称与参数已固定。字段借用 `&` 表示不转移数据所有权；返回新 GrantState 的设计让 C 可以先完成检查，再整体写回状态。

| 文件/负责人 | 固定签名 | 返回与行为 |
| --- | --- | --- |
| observations.rs / A | `parse_repo_id(body: &[u8]) -> AppResult<u64>` | 读取仓库数字 ID |
| observations.rs / A | `parse_release_page(body: &[u8]) -> AppResult<Vec<ReleaseRecord>>` | 解析一页，执行原始字段/时间校验，不筛窗口 |
| observations.rs / A | `fetch_releases(config: &GrantConfig) -> AppResult<FetchResult>` | 在 Instance 调用链中使用 Host HTTP，核对 repo_id、完整分页，失败无部分成功结果 |
| verification.rs / B | `build_candidate(config: &GrantConfig, round_id: u64, releases: &[ReleaseRecord]) -> AppResult<Candidate>` | 同 ID 同内容去重，冲突失败，筛窗口、排序、统计 |
| verification.rs / B | `validate_candidate(config: &GrantConfig, round_id: u64, candidate: &Candidate) -> AppResult<()>` | 校验完整配置、轮次、schema、范围、严格排序/唯一性、统计和字节上限 |
| verification.rs / B | `candidate_to_json(candidate: &Candidate) -> AppResult<String>` | compact JSON，按类型定义的字段顺序输出，不带空白与换行 |
| verification.rs / B | `candidate_from_json(input: &str) -> AppResult<Candidate>` | 先检查 16 KiB 上限再解析；未知字段与坏类型失败 |
| verification.rs / B | `verify_observation(config: &GrantConfig, round_id: u64, local: &ObservationState, proposed: &Candidate) -> AppResult<()>` | ready/轮次正确，并与本地候选逐字段相等 |
| grants.rs / C | `validate_config(config: &GrantConfig) -> AppResult<()>` | 检查 8.2 的所有约束；无外部 HTTP/时钟 |
| grants.rs / C | `initialize_grant(state: &GrantState, config: GrantConfig) -> AppResult<GrantState>` | 只允许一次，创建固定配置 |
| grants.rs / C | `advance_round(state: &GrantState) -> AppResult<GrantState>` | 已配置且未最终认证时轮次加一 |
| grants.rs / C | `apply_snapshot(state: &GrantState, candidate: Candidate, signer_node_ids: Vec<String>) -> AppResult<GrantState>` | 调用 B 的 validate_candidate，保存首次快照；签名者来自认证上下文，去重排序后须有 3–4 个 |
| grants.rs / C | `is_eligible(state: &GrantState) -> bool` | 有最终快照，且 stable_count 达到固定门槛 |
| grants.rs / C | `apply_claim(state: &GrantState, caller: &str) -> AppResult<GrantState>` | 校验真实调用者、资格、未领取，再登记积分 |

`verify_observation` 先调用 `validate_candidate`，再比较完整候选。A 的刷新入口顺序为 fetch_releases → build_candidate → 写本地观察。C 的接收入口顺序为 LDK 验证认证 → 取得实际 signer → apply_snapshot → 写 Network State。

公共辅助函数由 C 在 model.rs 提供：`api_ok<T>(data: T) -> ApiResponse<T>`、`api_error<T>(error: AppError) -> ApiResponse<T>`、`api_to_json<T: serde::Serialize>(response: &ApiResponse<T>) -> AppResult<String>`。金额仅为整数演示积分；本轮不引入 ERC20 精度或浮点计算。

### 8.5 对外入口、参数、返回值与权限

下表的 `ctx` 由 LDK 生成，不是用户传入的参数。所有普通对外方法使用 `export = eth`；返回 JSON 的 ABI 类型统一为 string，避免三人另外实现自定义结构体的 EthAbiType。Network 可写方法通过 cast send 发送，Instance 与只读查询通过 cast call 访问指定节点。此处写的是项目规定的最终接口，尚未实现，部署后需核对生成 ABI。[LDK 的导出与 ABI 类型](https://docs.lyquor.dev/docs/ldk/external/)

| 方法及参数（不含 ctx） | 类别/权限 | Rust 返回类型 | JSON data 或结果 | 负责人 |
| --- | --- | --- | --- | --- |
| `constructor()` | Network 构造器 | `()` | 初始化 admin 和默认状态，无构造参数 | C |
| `configure_grant(config_json: String)` | Network 可写，仅 admin | `LyquidResult<bool>` | 成功 true；拒绝返回业务错误，无状态变化 | C |
| `start_round()` | Network 可写，仅 admin | `LyquidResult<u64>` | 成功后的轮次 | C |
| `configure_committee(committee_json: String)` | Network 可写，仅 admin，一次性 | `LyquidResult<bool>` | 暂存四节点、阈值 3 的设置；另需激活同步 | B |
| `get_committee_status()` | Instance 只读，公开 | `LyquidResult<String>` | `ApiResponse<CommitteeView>` | B |
| `refresh_observation()` | Instance 可写，公开 | `LyquidResult<String>` | `ApiResponse<ObservationView>` | A |
| `get_observation()` | Instance 只读，公开 | `LyquidResult<String>` | `ApiResponse<ObservationView>` | A |
| `get_candidate()` | Instance 只读，公开 | `LyquidResult<String>` | `ApiResponse<Candidate>` | B |
| `request_certification()` | Instance 可写，公开 | `LyquidResult<String>` | `ApiResponse<CertificationSubmission>`；候选只能取本节点缓存 | B |
| `accept_snapshot(candidate_json: String)` | Network certified 入口，仅有效委员会认证 | `LyquidResult<bool>` | 首次保存快照成功 true；**没有 export=eth 的普通写入口** | C |
| `get_grant_status()` | Network 只读，公开 | `LyquidResult<String>` | `ApiResponse<GrantStatus>` | C |
| `claim()` | Network 可写，仅固定受益账户 | `LyquidResult<u64>` | 成功后累计积分；本项目首次为 100 | C |

普通 Network 写操作统一以运行时 `ctx.caller` 为授权主体：构造器把它保存为 admin，配置/开启轮次/配置委员会要求它等于 admin，领取要求它等于 beneficiary。认证内部 origin/caller 另按 8.6 固定，不能用 JSON 自报身份替代这些检查。B 的委员会配置复用同一管理员校验。查询可以公开。Instance 刷新只能取固定配置的 GitHub URL，认证发起不接受外部传来的任意候选。

**交易回执不是业务返回值。** cast send 确认提交至排序合约后，Lyquid 才异步执行。脚本轮询 get_grant_status/get_committee_status 确认配置、轮次、快照或积分变化；不要在交易回执中寻找上表的 u64/bool 业务返回。实例接口返回 submitted=true 后也必须查最终状态。轮询每秒一次、最多 60 秒；超时记录为“未确认”，重新查询后再决定是否重试。

### 8.6 认证调用的固定封装

本节固定 B/C 的接合点，避免两边各猜一个 group 或参数编码。

| 项目 | 固定约定 |
| --- | --- |
| topic | `releases` |
| 目标 | 当前 Lyquid 的 LVM 目标，使用其实际 sequence backend ID |
| 方法 | `accept_snapshot` |
| 参数 | 只有一个字段 `candidate_json: String`，使用 LDK 原生 `encode_by_fields!` / `decode_by_fields!` 对同名字段编码/解码 |
| 参数内容 | B 的 candidate_to_json 输出；节点要求解析后重新序列化与原字符串相同，再与本地 Candidate 比较 |
| origin | `Address::ZERO`，这是固定的认证内部来源约定，不用它授权用户领取 |
| caller | 当前 Lyquid ID 对应的运行时 Address |
| extra | 空 Bytes；需要绑定的信息全部在 candidate_json 内 |
| group suffix | None |
| 超时 | certify 的 timeout 参数固定 30,000 ms |
| B 的验证钩子 | 函数名 `validate`，参数 `ctx: &mut _`、`params: CallParams`、`extra: Bytes`、`target: OracleTarget`，返回 `LyquidResult<bool>`；group 为 `oracle::single_phase::releases` |
| C 的接收入口 | group 为 `oracle::certified::releases`，不得另开普通导出 |

验证钩子按官方单阶段约定核对 target、`params.group == "releases"`、method、Lyquor 原生 ABI、origin/caller、空 extra，然后调用 8.4 的纯校验函数。任一业务条件不符返回 `Ok(false)`。目标函数继续核对当前配置/轮次与首次入账条件，防止节点批准后状态已改变。

B 负责初始化、激活和同步的部署脚本；配置入口只允许一次有效 staging，阻止激活前被反复替换。使用 LDK 的 advance_epoch/finalize_epoch 机制，目标为 LVM。其生成 ABI 参数、节点 ID 编码和返回状态在项目部署时核验并填入脚本，不发明额外平台 API。[官方认证机制](https://docs.lyquor.dev/docs/ldk/oracle/)

### 8.7 固定错误码

message 可以补充可读原因；脚本只按 code 判断。业务错误不得返回成功计数 0。

| code | 触发条件 | retryable |
| --- | --- | --- |
| E_NOT_CONFIGURED | 项目尚未配置 | false |
| E_ALREADY_CONFIGURED | 重复设置项目或委员会 | false |
| E_UNAUTHORIZED | 配置或领取者无权限 | false |
| E_BAD_CONFIG | 配置违反字段约束 | false |
| E_BAD_INPUT | 参数、记录或时间字段不合法 | false |
| E_JSON | JSON 解析/序列化失败 | false |
| E_HTTP | 请求超时、连接失败或非限流的异常 HTTP 响应 | true，仅人工核对原因后重试 |
| E_RATE_LIMIT | 403/429 且响应显示限流 | true，到恢复时间后重试 |
| E_TOO_LARGE | 响应体、记录数或候选字节数超限 | false |
| E_INCOMPLETE | 达到第三页仍有下一页，无法完成全量取数 | false |
| E_REPO_MISMATCH | 实际仓库数字 ID 不同 | false |
| E_DUPLICATE_CONFLICT | 同 ID 有不同内容 | false |
| E_ROUND_MISMATCH | 候选/缓存轮次过时，或尚未开启轮次 | false |
| E_NOT_READY | 本节点没有完整候选 | true，先完成刷新 |
| E_CANDIDATE_MISMATCH | 候选与本地内容、计数或配置不一致 | false |
| E_FINALIZED | 已有不可变最终快照 | false |
| E_COMMITTEE_NOT_READY | 尚未完成委员会激活同步 | true，完成部署配置后重试 |
| E_CERT_REJECTED | 在超时内没有形成认证 | true，先核对各节点状态 |
| E_SUBMIT | 认证提交失败或未得到确定结果 | true，先查共享状态，避免误判未提交 |
| E_NOT_ELIGIBLE | 没有快照或未达到门槛 | false |
| E_ALREADY_CLAIMED | 已领取 | false |
| E_NOT_IMPLEMENTED | 尚未完成的开发占位函数 | false |
| E_INTERNAL | 不变量破坏、算术溢出、签名元信息异常等 | false |

Network 写入口将业务错误转为带有 `CODE: message` 的 LyquidError，错误分支不写状态；Instance/只读查询正常可处理的业务错误使用 ApiResponse。基础设施异常可以由 LDK 返回 RPC 错误，脚本需同时处理 RPC 失败和 JSON 内 ok=false。

**返回格式示意**，供实现和脚本断言使用，均非实测输出。成功提交认证的 JSON：

```json
{"ok":true,"data":{"grant_id":"grant-001","round_id":1,"submitted":true},"error":null}
```

未准备好候选的 JSON：

```json
{"ok":false,"data":null,"error":{"code":"E_NOT_READY","message":"Observation is not ready for the current round","retryable":true}}
```

初始共享状态查询格式如下；admin 在真实部署中替换为部署者地址：

```json
{"ok":true,"data":{"admin":"0x1111111111111111111111111111111111111111","state":{"config":null,"round_id":0,"snapshot":null,"claimed":false,"credits":0},"eligible":false},"error":null}
```

### 8.8 给三人共同使用的输入与预期

以下仅是人工构造的**离线测试数据**，不代表真实 GitHub 仓库，也不是已通过的测试结果。它使三人可以立即开发，不必等待 A 完成联网。

固定配置：grant_id=`grant-001`，repo_owner=`demo-owner`，repo_name=`demo-repo`，repo_id=123，window_start=1000，window_end=2000，min_stable_releases=2，beneficiary=`0x1111111111111111111111111111111111111111`，reward_credits=100，rule_version=1，round_id=1。

| id | published_at | draft | prerelease | 预期 |
| --- | --- | --- | --- | --- |
| 10 | 1000 | false | false | 窗口起点，计入正式版 |
| 11 | 1500 | false | false | 计入正式版 |
| 12 | 1600 | false | true | 保存依据，不计正式版 |
| 13 | 2000 | false | false | 窗口终点，排除 |

B 预期输出的 releases 为 10、11、12，stable_count=2。变换输入顺序不改变候选；重复同一条 10 去重；同 ID 10 改时间则报冲突；改候选 ID 但维持数量 2，应验证失败。C 在已认证该候选后计算 eligible=true，指定账户仅能领取一次 100。真实部署改用团队实际仓库、时间和账户，不能用上述伪仓库进行 live 演示。

契约修改只在发现明确错误或平台限制时进行，由 C 维护本节对应的 model.rs 与 interfaces.md，并在提交说明列出受影响调用。正常开发不需要另开数据结构设计会议。



## 2026-11-22 C 集成补充：平台适配与状态布局

- 新增内部 Network 状态 `committee_nodes: Vec<String>`，初始为空。首次 `start_round` 时从已经激活且与 staging 一致的 3/4 委员会固定四个规范化 Node ID；之后开轮要求委员会保持一致。没有新增业务 JSON 字段或改变任何业务 ABI 签名；持久状态布局发生变化，本版本须新部署，不承诺旧部署原地升级。
- LDK 0.8.4 自动导出的 `__lyquor_oracle_initialize` 在首次激活前可替换平台 staging，单靠项目包装函数不能禁止这一平台能力。应用独立保存批准的节点集合，C 的开轮和最终接收核对实际委员会恰为该集合及阈值 3；B 的原有查询、提交和验证钩子保持不变。内置入口不能重写应用的 pin；偏离配置会被业务拒绝。creator 仍能破坏激活可用性，不能宣称可抵抗恶意部署管理员。
- `accept_snapshot` 仍只存在于 `oracle::certified::releases`。除 LDK 自动验签外，接收端检查固定 origin/caller、当前 source 委员会、证书 config hash/epoch 和当前配置/轮次。没有添加普通入账入口。
- 锁定 LDK 的 `verify_oracle_cert_signatures` 仅验前 `threshold` 个签名。因此本版本只从真实 `ctx.cert.signers` 的前三个已验签 ID 解析、排序并保存 signer Node ID；不把附带的第四个 ID 当作额外签名证据。纯状态函数仍兼容 3–4 个已验证、去重后的 signer，运行时输出固定记录已验证的三个。
- C 的开轮若委员会尚未激活或不匹配返回 `E_COMMITTEE_NOT_READY`；B 的认证入口错误码保持原实现。提交结果不确定时先查询共享状态。
- B 的 `get_candidate` 在未就绪或旧轮返回 `E_NOT_READY`。配置冻结后的重新配置返回 `E_FINALIZED`，配置后尚未冻结的重复配置返回 `E_ALREADY_CONFIGURED`。
- CLI 接口和 A/B/C 适配器参数以 `scripts/README.md` 为准。C 的独立适配脚本分别确认 staging、目标 epoch 和四节点 active，不能只判断调用返回的交易回执。

平台依据：Cargo.lock 固定 LDK `47a5ce17ea756af1946a5bc0f71d01b32b3b4242` 的 `lyquid/src/runtime/oracle/{mod,source,dest}.rs`、`runtime/syntax.rs` 与实际生成 Solidity ABI。没有修改 LDK 缓存或版本。
