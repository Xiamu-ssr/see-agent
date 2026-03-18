# CLAUDE.md

## 用户对齐心智文档
MentalModel.md 是和用户对齐的的心智文档。代码实现应该 >= 这个文档。
ImplicitContractDefense.md 是隐式契约防御的介绍，ai coding代码实现可以参考这个文档。

## TDD开发流程
1. 先写测试（按功能模块粒度，不是单函数），跑 `cargo test` 确认编译通过但断言失败
2. 实现代码，跑 `cargo test` 确认绿灯
3. 需要时重构，保持绿灯
4. 测试必须覆盖实际代码路径，不是理想调用方式

## 改完代码必须验证
改完代码跑一次质量门禁：
```bash
bash scripts/check.sh
```
check.sh = clippy (native) + clippy (wasm) + cargo test + cargo build。
TDD 循环内用 `cargo test` 快速迭代，最终提交前跑 check.sh 全量检查。

## 提交代码
验证通过后，commit 并 push

## 实施约束
- 不做兼容：旧代码旧结构直接删，不保留向后兼容逻辑
- 必须删旧代码：如果新设计不需要某个字段/函数/文件，直接删除
