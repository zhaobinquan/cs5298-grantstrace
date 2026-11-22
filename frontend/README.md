# GrantsTrace 本地演示前端

这个页面直接调用当前 Lyquid 的四个 HTTP JSON-RPC 端点，无需 npm 构建或中间服务。仓库中的 `feature/frontend` 基于 `feature/b-integration`；本地 devnet 需要运行包含 B/C 联调接口的 Lyquid，单独的 `main` 占位版本不能提供完整流程。

## 启动

在仓库根目录运行：

```bash
python3 -m http.server 4173 --directory frontend
```

打开 `http://127.0.0.1:4173/`。页面默认填写本次四节点实验的端口与 Lyquid 地址；若 devnet 重建，请将地址改为新的 `shaker deploy` 输出。节点 RPC 必须允许浏览器跨域请求；本地 Lyquor 0.8.4 的 `/api` 已返回 `Access-Control-Allow-Origin: *`。

## 功能

- 读取 `get_grant_status`、`get_committee_status`、四节点 `get_observation` 与已就绪节点的 `get_candidate`。
- 对单节点或四节点依次调用 `refresh_observation`，从节点 1 调用 `request_certification`，并轮询最终快照。
- 通过 EIP-1193 浏览器钱包调用 `configure_grant`、`configure_committee`、`start_round` 和 `claim`；委员会 epoch 激活调用 LDK 的 `__lyquor_oracle_advance_epoch`、`__lyquor_oracle_finalize_epoch`。激活所需 LVM 地址由 `shaker to-hex LYQUID_ID` 得到。
- 写操作以最终查询到的业务状态为准。`eth_sendTransaction` 返回哈希或 receipt 成功不代表 Lyquid 异步业务函数已经执行成功。

首次演示建议使用现有部署，只读查看四节点和快照；配置、轮次和领取按钮会实际修改本地 devnet。页面不存储私钥，钱包账户须是本地 devnet 部署者或配置的受益人。资助配置文本是可编辑示例，请核对仓库 ID、时间窗口与受益地址后再签名。

此页面仅用于本机四节点演示。公网托管页面无法直接连接用户电脑的 `127.0.0.1` 节点；要远程使用须另行部署可访问的 RPC 并做好访问控制。
