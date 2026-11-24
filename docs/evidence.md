# 验证记录

2026-11-19：用户终端截图确认 cargo check 退出码 0；shaker build Cargo.toml 显示 Build success 且退出码 0。此为骨架构建证据，不是完整业务运行证据。

同日首次 shaker test Cargo.toml 输出 running 0 tests、0 passed；命令退出码虽为 0，但没有执行用例，不能算测试通过。将标注改为 #[lyquid_test::test] 后，用户终端截图确认实际执行 5 项，5 passed、0 failed，退出码 0，用例运行耗时 1.78 秒。

区分三类证据：静态检查/编译结果、人工输入的单元测试、真实 devnet/API 集成实验。
每次记录日期、提交 SHA、命令、输入来源、预期、实际输出和是否通过。
## 已通过的骨架单元测试（2026-11-19）

| 用例名 | 验证内容 | 结果 |
| --- | --- | --- |
| json_preserves_u64_ids_and_candidate_field_order | u64 最大值与候选 JSON 字段顺序 | 通过 |
| null_is_allowed_but_missing_option_keys_are_rejected | 可空字段允许 null，但不能缺失键 | 通过 |
| response_envelopes_have_explicit_null_fields | 成功/错误响应的字段组合 | 通过 |
| eligibility_requires_matching_config_round_and_threshold | 资格要求配置、轮次与门槛匹配 | 通过 |
| candidate_rejects_unknown_and_missing_fields | 拒绝候选中的未知字段和缺失字段 | 通过 |

输入来源：fixtures/offline_candidate.json 人工样本与测试内构造状态。证据来源：C 提供的终端截图；此时尚未首次提交，提交 SHA 待首个 commit 产生后关联。没有将这些单元测试描述为在线认证实验。

2026-11-22 已补充四节点托管、委员会激活、真实 GitHub 取数、低于阈值认证失败、一次领取和重复领取拒绝的本地 devnet 实测，详见 [B 的贡献与验证](contribution-b.md)。错误候选拒绝由同数量不同记录的单元测试证明；公开入口不接受外部提案，因此未构造 devnet 假提案。



## 2026-11-22 C 接手验证（提交前工作树）

基于 B 的 `47eb9cb`，保留 A/main `d12e83d` 的实现。C 本次格式、locked check、Shaker build 退出 0；guest tests 42 passed / 0 failed；Python 离线脚本测试在后续调整前为 19 passed，当前版本 20 passed。真实四节点完成配置/激活、2 节点拒绝/3 节点成功、一次领取 100 积分、权限、旧轮次、最终冻结与重复领取反例，实际错误码与状态不变同时核对。

此前一次 `test_devnet.py` 因门槛 3 的部署遇 GitHub `E_RATE_LIMIT` 退出 1；该版本的结果不能替代当前代码的验收。部署 ID、不同构建的镜像摘要、实际输入和诊断记录详见 [C 的贡献与验证](contribution-c.md)。没有用旧骨架 5 项测试替代新验证，也没有创建本次不存在的提交 SHA。

用户要求保留 A/B 文件原样后，C 撤回对 A 的 e2e 和 B 的 API/脚本修改，转用 C 独立适配器并将委员会绑定移入 C 的开轮入口。以上早期 live 结果对应此前本地版本。当前代码镜像在新部署上完成全部主流程：第一次 `test_devnet.py` 在合格部署第二轮取数遇 `E_RATE_LIMIT` 退出 1；配额恢复后同一部署的 `--resume-qualified` 进程退出 0，并完成合格快照、领取、拒绝反例，以及另一份门槛 3 的不合格快照与 `E_NOT_ELIGIBLE`。随后全新部署完整连续运行 `test_devnet.py`，进程退出 0。A 原版 e2e 搭配 C 适配器在另一个全新部署退出 0，最终 `eligible=true`、`claimed=true`、`credits=100`。各段运行、镜像摘要及版本边界见 [C 的贡献与验证](contribution-c.md)。
