# 测试输入待补充

A 维护原始 GitHub 响应与脱敏样本。人工构造的离线输入必须明确标记。
offline_candidate.json 是按 docs/interfaces.md 第 8.8 节人工构造的候选对象，仅供类型/序列化单元测试。
它不是 GitHub 原始响应，不包含真实签名，不能当作认证成功证据。
本版没有实时 API 样本，不包含在线实验结果。

`offline_github_repo.json` 和 `offline_github_releases.json` 是人工构造的 GitHub API 形状样本，用于 A 的离线解析测试。仓库样本使用 `u64::MAX` 检查整数精度；发布样本覆盖带偏移量的时间、预发布、草稿、空发布时间和未知字段。

