# CLAUDE.md

本文件为 Claude Code 在本仓库中工作时提供指导。

## 项目简介

**Flux** 是面向大数据场景的分布式计算引擎，**全面对标 Apache Spark**（批 + SQL + 流 + ML + 图，按里程碑分阶段交付）。核心主张：Rust 自研内核的原生性能与资源效率 + Go 控制面的简单运维 + 对象存储优先的存算分离。

### 既定产品决策（勿重新讨论；变更须走需求/ADR 流程）

| # | 决策 |
| --- | --- |
| D1 | 能力范围：全面对标 Spark，按 M0–M7 里程碑分阶段交付 |
| D2 | 内核路线：Rust **完全自研**（解析器/优化器/执行器/Shuffle 协议）；仅复用数据格式标准库（arrow-rs、object_store、tokio/tonic） |
| D3 | 部署形态：自研 Master(Go) / Worker(Rust) 独立集群（对标 Spark Standalone）；K8s Operator 后置 M7 |
| D4 | 存储策略：对象存储优先（S3/OSS/MinIO + Parquet），存算分离，计算节点无状态 |
| D5 | 双形态共用内核：单机嵌入式模式与集群模式走同一执行路径（ADR-008） |

## 语言边界（铁律，违反即架构事故）

- **Rust = 数据面**：SQL 解析、查询优化、向量化执行、Shuffle、内存管理与 Spill。
- **Go = 控制面**：集群管理、作业调度、目录元数据、REST/gRPC 服务、可观测性、运维工具。
- **Go ↔ Rust 通信（传输铁律，ADR-011）**：契约与传输解耦——消息契约仍是 Protobuf（控制流）+ Arrow IPC（数据流），但**同机通信一律共享内存 IPC，禁止 TCP 回环（127.0.0.1）**：控制流走 shm 环形缓冲 RPC 帧（eventfd 通知），数据流走 shm Arrow 零拷贝区域；**网络（gRPC/Arrow Flight）仅用于跨机场景**（物理上必须网络的部署）。共享内存是 IPC 不是 FFI——进程边界与故障域隔离不变，**任何形式的 FFI 仍禁止**。
- Rust 不引入集群管理职责（etcd/k8s client 等）；Go 不触碰数据内容解析与算子执行（不引 arrow/parquet 计算依赖）。
- 共享契约（错误码、指标名、物理计划结构）只定义于 `proto/`，两侧生成，禁止手抄两份。
- 架构规则全表（AR-1~13）与依赖白名单见[架构与代码维护文档 第 2 章](docs/05-架构与代码维护文档.md)。

## 关键技术速查（与详细设计保持一致，改动须双侧同步）

| 项 | 值 |
| --- | --- |
| 端口 | Master：gRPC 9090 / HTTP 8080 / metrics 9095；Worker：gRPC 9091 / Flight 9092 / metrics 9096（网络端口仅跨机通信需要；同机 Go↔Rust 走 shm 无端口占用） |
| Go↔Rust 传输 | 同机：**shm**（控制环形缓冲 + Arrow 零拷贝区域，eventfd 通知，ADR-011）；跨机：gRPC + Arrow Flight；降级：UDS（非 TCP）；设计见详细设计 12.6 |
| 数据格式 | 内存与交换 Arrow IPC；落盘 Parquet（zstd-3）；控制流 Protobuf |
| 关键默认值 | 批 8192 行；Broadcast 阈值 64MiB；心跳 1s / 摘除 15s；Task 重试 3 次（退避 1s/5s/15s）；小结果阈值 32MiB；行组 128MB |
| 错误码段 | 1xxx 语法语义 / 2xxx 计划 / 3xxx 执行 / 4xxx 集群调度 / 5xxx 存储 IO / 6xxx 安全 / 7xxx 配置内部 |
| ID 格式 | `job_<ulid>`、`w_<uuid>`、`sess_<uuid>`、Stage `s{idx}`、Task `{stage}-{partition}-{attempt}` |
| SQL 编译位置 | 发生在数据面（Worker 的 PlanService）；Master 不解析 SQL，只见序列化物理计划（`plan_version`） |

## 仓库结构

项目处于 **M0（脚手架与预研）阶段**，代码未落地。目标结构：

布局原则：顶层按**职责域**组织（数据面 / 控制面 / 端到端测试 / 基准 / 运维交付物），语言由目录内工具链文件体现；Rust crate **目录名 = 包名去 `flux-` 前缀**；`migrations` 跟随其所属模块（statestore）。

```text
flux/
├── CLAUDE.md / README.md / CHANGELOG.md
├── docs/                      # 需求与设计文档（01–06）+ adr/ + tech-debt.md + sql-compat.md
├── proto/flux/v1/             # 唯一契约源（路径与 package flux.v1 对齐；buf 管理）
├── engine/                    # 数据面 · Rust workspace
│   ├── Cargo.toml             # [workspace.dependencies] 统一版本
│   ├── crates/                # 目录名 = 包名去 flux- 前缀
│   │   ├── common/            #   flux-common：错误码、配置、ID、指标常量
│   │   ├── sql/               #   flux-sql：词法/语法（手写递归下降）/语义分析
│   │   ├── plan/              #   flux-plan：逻辑计划、优化器（RBO+CBO）、物理计划、proto 序列化
│   │   ├── exec/              #   flux-exec：向量化执行引擎、算子、表达式、内存池与 Spill
│   │   ├── storage/           #   flux-storage：对象存储抽象、Parquet 读写、提交协议
│   │   ├── shuffle/           #   flux-shuffle：分区器、桶管理、Arrow Flight 服务
│   │   ├── shmipc/            #   flux-shmipc：共享内存 IPC（shm 环形缓冲 + Arrow 零拷贝区，ADR-011）
│   │   ├── worker/            #   flux-worker：Worker 二进制（PlanService/TaskService/心跳）
│   │   └── client/            #   flux-client：Rust SDK（LocalSession / ClusterSession）
│   ├── fuzz/                  # cargo-fuzz 目标
│   └── tests/sqlsem/          # 语义测试用例库（yml，单机/集群双端复跑）
├── control/                   # 控制面 · Go module
│   ├── cmd/flux-master/       # Master 入口
│   ├── cmd/flux/              # CLI 入口
│   ├── internal/              # apiserver / jobmanager / scheduler / workermanager /
│   │                          # shmipc（Go 侧 shm IPC）/ catalog / statestore / resultstore / observability / config
│   └── migrations/            # StateStore schema 迁移（跟随 statestore）
├── tests/                     # 跨组件端到端测试
│   ├── integration/           # 本地多进程集群（1 master + 2 worker）
│   ├── chaos/                 # 故障注入
│   ├── soak/                  # 72h 长稳
│   └── compat/                # Master↔Worker 新旧组合冒烟
├── benchmarks/                # 端到端基准：TPC-H/TPC-DS 脚本 + BUDGET.md
│                              # （criterion 微基准按 cargo 惯例在各 crate 的 benches/ 内）
├── deploy/                    # 运维交付物：systemd 单元、docker-compose（dev-cluster）、
│                              # Grafana 看板 JSON、示例配置（master.toml / worker.toml）
├── scripts/                   # 开发辅助：脚手架、codegen、release 辅助
├── ui/                        # Web UI（M3）
└── .github/workflows/         # CI（rust / go / proto / arch / bench / release）
```

crate 依赖方向：`worker/client → plan/exec/shuffle → sql/storage → common`，禁止反向与循环。

## 里程碑与当前状态

**当前：M0（脚手架与预研）** —— 文档集已定稿，下一步为仓库骨架、proto 契约、CI 与三个技术 Spike。

M1 单机引擎 MVP → M2 分布式批处理 → M3 生产可用 → M4 生态与多租户 → M5 流处理 → M6 ML 与图 → M7 K8s Operator。各里程碑退出标准见 SRS 第 8 章。

## 常用命令（M0 脚手架建立后生效）

| 命令 | 作用 |
| --- | --- |
| `make build` | 构建 Rust workspace + Go 二进制 |
| `make test` / `make lint` | 双侧测试 / fmt + clippy + golangci-lint + buf lint |
| `make proto` | proto 双侧生成与一致性校验（`make proto-check`） |
| `make dev-cluster` | 本地拉起 1 master + 2 worker |
| `make bench` / `make chaos` | 微基准 + TPC-H 冒烟 / 故障注入用例 |

## 文档索引（docs/，中文）

| 文档 | 回答的问题 |
| --- | --- |
| [01-软件需求规格说明书](docs/01-软件需求规格说明书.md) | 做什么：用例模型、FR/NFR 逐条规格（输入/处理/验收）、数据字典、接口需求、验收测试规格 |
| [02-PRD文档](docs/02-PRD文档.md) | 为什么做：定位、竞品、用户故事、路线图、指标体系、风险登记册 |
| [03-概要设计说明书](docs/03-概要设计说明书.md) | 总体长什么样：架构与 ADR-001~008、模块清单、RPC 清单、时序图、故障模式矩阵 |
| [04-详细设计说明书](docs/04-详细设计说明书.md) | 具体怎么做：SQL 文法、类型/函数目录、计划结构、算子伪代码、proto 全文、配置/指标/错误码全表 |
| [05-架构与代码维护文档](docs/05-架构与代码维护文档.md) | 架构如何守护：AR 规则、ADR 流程、版本兼容、CI、技术债、RACI |
| [06-代码规范与开发指南](docs/06-代码规范与开发指南.md) | 怎么写代码：Rust/Go 规范（含 ✅/❌ 示例）、测试规范、PR 模板、评审清单 |

**按任务选读**：实现新算子/SQL 功能 → 04 第 3–6 章 + 06 第 3 节；改 proto 或跨语言行为 → 04 第 12 章 + 05 第 3 章（ADR）；新增依赖 → 05 第 2.4 节白名单；需求疑问 → 01 对应 FR 编号。

## 开发约定

- **语言**：docs/ 中文；代码、注释、提交信息、issue 英文。
- **流程**：Conventional Commits（`type(scope): subject`，scope = crate/包名）+ trunk-based 短分支；禁止直 push `main`。
- **文档纪律**：改代码不改文档视同未完成——命中 ADR 触发条件（05 文档 3.1 节）的变更必须同步对应文档。
- **动手前必读**：[代码规范与开发指南](docs/06-代码规范与开发指南.md)（第 3/4 节按语言）与[概要设计](docs/03-概要设计说明书.md)第 3–4 章。
- **待决策开放问题**（勿自行定案）：项目名与重名（PRD Q4）、Go module path（规范 Q1）、CLI 框架选型（TD-004）。
