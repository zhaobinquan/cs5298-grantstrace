# 运行与验收（Ubuntu / WSL）

需要锁定版本的 Shaker、Cast、Python 3 和已启动的四节点 Lyquor devnet。所有 `.sh` 通过 `bash scripts/文件名.sh` 调用。

## 脚本与参数

| 脚本 | 必需输入 | 成功确认 |
| --- | --- | --- |
| `deploy.sh`（C） | `ALLOW_DEPLOY=1`、`ADMIN_KEY` | 新建部署；四节点构造状态及 admin 一致；可选自动配置 |
| `configure.sh`（C） | `GRANTS_ADDRESS`、`ADMIN_KEY`、`GRANT_CONFIG_FILE` | 完整配置及初始状态匹配 |
| `start-round.sh`（C） | `GRANTS_ADDRESS`、`ADMIN_KEY` | 委员会激活后 round 恰好加一，其余业务字段不变 |
| `c-committee.sh`（C 独立适配器） | `GRANTS_ADDRESS`、`ADMIN_KEY`、`NODE_IDS_JSON` | staging → 目标 epoch → finalize → 四节点 active |
| `observe.sh`（A） | `GRANTS_ADDRESS`、`RPC1…RPC4` | 四节点完整候选相等 |
| `c-certify.sh`（C 独立适配器） | `GRANTS_ADDRESS` | 已提交，且最终快照与本地完整候选一致 |
| `claim.sh`（C） | `GRANTS_ADDRESS`、`BENEFICIARY_KEY` | claimed=true，积分恰加奖励，快照和配置不变 |
| `e2e.sh`（A 原版） | 下文适配器变量 | 编排全部步骤，最终积分正确 |

C 的适配器共用 `grants_cli.py`；A/B 原脚本未修改。兼容 B 原有的 `LYQUID_ADDRESS`、`RPC_URL`、空格分隔 `RPC_URLS`；若同时设置新旧变量，值必须一致。`LYQUID_ID` 与 EVM 合约地址是不同标识，不能互换。

默认四端点为 `http://127.0.0.1:10087/api`、11087、12087、13087。自定义时设置全部 `RPC1…RPC4`。部署另使用 `LYQUOR_ENDPOINT`（默认 `ws://127.0.0.1:10087/ws`）、`LYQUID_REFERENCE`（默认 `http://127.0.0.1:8000/lyquids`）。代理环境需让 localhost 请求直连，例如 `export NO_PROXY=127.0.0.1,localhost no_proxy=127.0.0.1,localhost`。

`GRANT_CONFIG_FILE` 指向完整 GrantConfig JSON，字段见 `docs/interfaces.md`。部署时若设置该变量会自动配置；否则单独运行 configure。部署只新建，不提供覆盖旧应用的 update 选项。可通过 `DEPLOY_ADDRESS_FILE`、`DEPLOY_INFO_FILE` 保存地址与 Shaker JSON（contract、lyquid_id、os_version）。输出文件在业务等待前写出，超时后先检查它们和共享状态，避免重复部署。

脚本不打印私钥或原始 RPC 错误；本地私钥只从环境读取，仍会作为 Cast/Shaker 子进程参数传递，仅用于本地测试账户。不要提交 `.env`、日志、真实凭据或节点数据。交易回执不是业务成功，正常状态转换以一秒一次、最多 60 秒的共享状态轮询为准；认证 RPC 另有 40 秒客户端上限，部署构建另有 180 秒上限。失败不自动重发交易。

## 本地四节点

Docker Desktop 启用当前 Ubuntu 的 WSL integration 后：

```bash
docker compose -p grantstrace-c-devnet -f "$HOME/.shakenup/ldk/docker/multi/docker-compose.yaml" up -d
docker compose -p grantstrace-c-devnet -f "$HOME/.shakenup/ldk/docker/multi/docker-compose.yaml" ps -a
```

等待 setup-devnet 退出 0。独立 project 名保留原有节点数据；不要执行删除卷的清理命令。使用已有不同端口测试网时调整环境变量。

在 Ubuntu 终端设置实际测试账户（admin 和 beneficiary 建议不同），然后准备历史窗口配置：

```bash
read -rsp 'Local devnet admin key: ' ADMIN_KEY; echo; export ADMIN_KEY
read -rsp 'Local devnet beneficiary key: ' BENEFICIARY_KEY; echo; export BENEFICIARY_KEY
export RPC1=http://127.0.0.1:10087/api RPC2=http://127.0.0.1:11087/api
export RPC3=http://127.0.0.1:12087/api RPC4=http://127.0.0.1:13087/api
export NO_PROXY=127.0.0.1,localhost no_proxy=127.0.0.1,localhost
export GRANT_CONFIG_FILE=/tmp/grant-config.json
python3 - <<'PY'
import json, os, subprocess
beneficiary = subprocess.check_output(['cast','wallet','address','--private-key',os.environ['BENEFICIARY_KEY']], text=True).strip().lower()
config = dict(grant_id='grant-001', repo_owner='jqlang', repo_name='jq', repo_id=5101141,
              window_start=1748736000, window_end=1754006400, min_stable_releases=2,
              beneficiary=beneficiary, reward_credits=100, rule_version=1)
with open(os.environ['GRANT_CONFIG_FILE'], 'w') as out:
    json.dump(config, out)
PY
```

上述 jq 历史窗口是集成测试输入，不是团队自己发布的软件；本次实测有两个正式 Release。公开 GitHub 内容仍可能改变。最终演示可替换为团队实际冻结的仓库与窗口，不要使用 offline fixture 的虚构 repo_id 联网。

读取实际四个节点 ID（响应格式来自锁定版本 GetNodeInfo）：

```bash
export NODE_IDS_JSON="$(python3 - <<'PY'
import json, os, urllib.request
ids = []
for i in range(1, 5):
    base = os.environ[f'RPC{i}'].removesuffix('/api')
    req = urllib.request.Request(base + '/lyquor.node.v1.NodeService/GetNodeInfo', data=b'{}', headers={'Content-Type':'application/json'})
    with urllib.request.urlopen(req, timeout=10) as response:
        ids.append(json.load(response)['nodeId']['value'])
print(json.dumps(ids))
PY
)"
```

## 全流程

首次部署并完成全部步骤：

```bash
export ALLOW_DEPLOY=1
export DEPLOY_SCRIPT=scripts/deploy.sh CONFIGURE_SCRIPT=scripts/configure.sh
export COMMITTEE_SCRIPT=scripts/c-committee.sh START_ROUND_SCRIPT=scripts/start-round.sh
export CERTIFY_SCRIPT=scripts/c-certify.sh CLAIM_SCRIPT=scripts/claim.sh
bash scripts/e2e.sh
```

fresh deployment 时 A 的 e2e 传递合约地址，C 的委员会适配器从节点托管列表解析 Lyquid ID。已有部署设置 `GRANTS_ADDRESS`，可选设置 `LYQUID_ID`；e2e 要求尚未生成快照且未领取。若要逐步运行，先运行 deploy 并从 JSON 输出取两个标识，再依次运行 committee、start-round、observe、certify、claim。

## 测试

```bash
cargo fmt --check
cargo check --locked
shaker build Cargo.toml
shaker test Cargo.toml
python3 scripts/test_observe.py -v
python3 scripts/test_grants_cli.py -v
```

`test_devnet.py` 创建全新部署，用真实 GitHub/委员会验证 2 节点拒绝、3 节点成功、旧轮次、普通入账路径拒绝、C 权限/冻结/重复领取，以及另一份门槛更高的不合格快照。需要上面的配置、不同的两个测试账户与四节点参数。为读取实际拒绝原因，应为测试 node1 启用 `LYQUOR_LOG=debug`；可通过独立 Compose override 设置该环境变量，保留原数据卷。测试不会自行启动、重置或删除节点。

```bash
export ALLOW_DEPLOY=1 DEVNET_NODE_CONTAINER=grantstrace-c-devnet-node1-1
python3 scripts/test_devnet.py
```

这是 live 测试，成功不能由 mock 或纯函数测试替代。运行结果、镜像摘要和限制见 `docs/contribution-c.md`、`docs/evidence.md`。


遇到 E_RATE_LIMIT 时保留现有部署，读取 `get_observation` 的 error.message 中 X-RateLimit-Reset，不循环重发取数。若合格场景已进入尚无快照的第二轮，配额恢复后可用 `test_devnet.py --resume-qualified <已部署地址>` 完成该部署并继续新建不合格场景。若不合格场景已部署且尚无快照，可用 `--resume-ineligible <已部署地址>` 单独完成它。恢复只适用于同一代码镜像与预期配置；内部状态布局变化后必须新部署。
