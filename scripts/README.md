# 运行脚本

- `committee.sh`：需要 `LYQUID_ADDRESS`、`LYQUID_ID`、`ADMIN_KEY`、`NODE_IDS_JSON`；默认向节点 1 提交配置，并逐一确认 10087、11087、12087、13087 四节点已激活。不同端口可通过 `RPC_URL` 和空格分隔的 `RPC_URLS` 覆盖。
- `certify.sh`：需要 `LYQUID_ADDRESS`；从节点 1 的已就绪候选发起认证，并在 60 秒内确认共享快照。
- `observe.sh`、`e2e.sh`：A 的观察及流程脚本。
- `deploy.sh`、`claim.sh`：C 的部署及领取脚本。

本地四节点部署示例：`shaker deploy Cargo.toml -r http://127.0.0.1:8000/lyquids`。先配置 grant、激活委员会、开启轮次，再在至少三个节点刷新观察并认证。`ADMIN_KEY` 使用本地 devnet 帐户，真实凭据不得提交到 Git。若 `E_HTTP`，检查容器内 `api.github.com` 的 DNS 与连通性；不要将临时 IP 写入业务代码。
