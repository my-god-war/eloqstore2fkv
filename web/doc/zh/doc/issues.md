# 审查记录

`issue/` 是迁移期间的审查看板：

- `new/`：新意见入口。
- `todo/`：已采纳、待开发。
- `done/`：已开发并有测试证据，待复审。
- `reviewed/`：已复审归档。
- `reject/`：不采纳，写明理由。

流程见 `issue/new.md`、`issue/run.md`、`issue/review.md`。

审查聚焦 fkv 与 C++ 实现的行为差异、格式兼容、性能与代码结构。每个意见一个
文件，避免批量移动。
