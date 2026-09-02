# Flux

**面向大数据场景的分布式计算引擎 —— 全面对标 Apache Spark。**

> 用 Rust 自研内核获得原生性能与资源效率，用 Go 控制面获得简单的集群运维体验，用对象存储优先的存算分离贴合云上成本模型。目标：同等作业，比 Spark **更快、更省、更易运维**。

## 为什么做这个

Spark 统治大数据十余年，但 JVM 遗产始终未根治：内存利用率低（对象头/装箱/GC 预留）、作业启动慢、Shuffle 序列化开销大、运维门槛高。Arrow 生态与原生内核（Polars、DataFusion、Velox）已证明：列式 + 无 GC + SIMD 可以带来数倍的性能/资源优势——但开源世界还没有一个**完整的、分布式的、对标 Spark 全域能力的**新引擎。Flux 想补上这个位置。

## 核心特性

- **Rust 自研内核（数据平面）**：SQL 解析、查询优化、向量化物理执行、Shuffle、内存管理与 Spill——不依赖 DataFusion/Velox 等执行框架，性能天花板与演进自由度完全自主。
- **Go 控制面**：集群管理、作业调度、目录元数据、REST/gRPC 服务与运维工具，单二进制部署，运维门槛从"大数据工程师"降到"后端工程师"。
- **同机零回环通信（ADR-011）**：Go↔Rust 同机链路走共享内存 IPC（控制 shm 环形缓冲 + 数据 Arrow 零拷贝区域），无 TCP 回环协议栈开销；网络仅用于跨机场景。
- **对象存储优先的存算分离**：数据与结果在 S3/OSS/MinIO（Parquet），计算节点无状态，即插即用弹性伸缩。
- **双形态共用内核**：单机嵌入式模式（开发/轻量场景，类 DuckDB 体验）与自研 Master/Worker 分布式集群（对标 Spark Standalone），同一条执行路径。
- **长期路线覆盖 Spark 全域**：批 + SQL（M1–M3）→ 流处理（M5）→ 机器学习与图计算（M6），K8s Operator 后置（M7）。

## 架构总览

```text
   ┌─────────────────────────────┐
   │     flux CLI / Rust SDK     │  客户端
   └──────────────┬──────────────┘
                  │ gRPC / REST
   ┌──────────────▼──────────────┐
   │   flux-master（Go）          │  控制平面：集群 / 调度 / 目录 / API
   └──────────────┬──────────────┘
                  │ 同机 shm IPC ｜ 跨机 gRPC（ADR-011）
   ┌──────────────▼──────────────┐
   │   flux-worker（Rust）        │  数据平面：SQL 编译 / 执行 / Shuffle
   └──────────────┬──────────────┘
                  │ Parquet（S3 API）
   ┌──────────────▼──────────────┐
   │   S3 / OSS / MinIO 对象存储  │  数据与结果
   └─────────────────────────────┘

   Worker ↔ Worker：同机本地直读 ｜ 跨机 Arrow Flight
```

设计要点：SQL 编译发生在数据面（Master 不理解 SQL，只见序列化物理计划）；Shuffle 数据不经过 Master；开放标准作为兼容面（SQL / Parquet / Arrow IPC / Prometheus）。

## 快速开始（规划中，M1 起可用）

```bash
# 单机模式：直接查询 Parquet（M1）
flux sql --local -e "SELECT count(*) FROM 's3://bucket/lineitem/*.parquet'"

# 集群模式：自研 Master/Worker（M2）
flux-master start --config master.toml
flux-worker start --config worker.toml --master <master-addr>
flux sql -e "INSERT INTO sales.daily SELECT ..."
```

> 项目处于 **M0（脚手架与预研）阶段**，以上命令尚未实现；当前交付为完整的需求与设计文档集（见下）。

## 路线图

| 里程碑 | 内容 | 状态 |
| --- | --- | --- |
| M0 脚手架与预研 | 文档集、仓库骨架、CI、技术 Spike（扫描/IPC/执行模型） | **← 当前** |
| M1 单机引擎 MVP | SQL 前端、向量化内核、Parquet/对象存储、CLI 单机模式、TPC-H 10GB | |
| M2 分布式批处理 | Master/Worker、Shuffle 全策略、容错重试、指标日志 | |
| M3 生产可用 | CBO、分区表、Web UI、Master HA、TLS、性能对标报告 | |
| M4 生态与多租户 | Iceberg、UDF、HDFS/JDBC、队列与 RBAC | |
| M5 流处理 | 微批、事件时间/水位、exactly-once、CDC | |
| M6 ML 与图 | Flux ML（线性/树模型）、Flux Graph（BSP） | |
| M7 云原生深化 | K8s Operator、自动扩缩容 | |

## 仓库结构

```text
engine/    数据面（Rust workspace）：common / sql / plan / exec / storage / shuffle / shmipc / worker / client
control/   控制面（Go module）：flux-master、flux CLI、internal/*
proto/     跨语言契约（flux.v1，buf 管理）
docs/      需求与设计文档；tests/ 端到端测试；benchmarks/ 基准；deploy/ 运维交付物
```

完整布局与布局原则见 [CLAUDE.md](CLAUDE.md)。

## 文档

| 文档 | 回答的问题 |
| --- | --- |
| [软件需求规格说明书](docs/01-软件需求规格说明书.md) | 做什么：用例模型、逐条 FR/NFR 规格（含验收标准）、数据字典、接口需求 |
| [PRD 文档](docs/02-PRD文档.md) | 为什么做：定位、竞品、用户故事、路线图、指标体系、风险 |
| [概要设计说明书](docs/03-概要设计说明书.md) | 总体长什么样：架构与 ADR-001~011、模块清单、RPC 清单、时序图、故障矩阵 |
| [详细设计说明书](docs/04-详细设计说明书.md) | 具体怎么做：SQL 文法、函数目录、算子伪代码、完整 proto、配置/指标/错误码全表 |
| [架构与代码维护文档](docs/05-架构与代码维护文档.md) | 架构如何守护：AR 规则、ADR 流程、版本兼容、CI、技术债、RACI |
| [代码规范与开发指南](docs/06-代码规范与开发指南.md) | 怎么写代码：Rust/Go 规范（含示例）、测试规范、PR 模板、评审清单 |

## 参与贡献

当前处于文档评审与 M0 筹备阶段。动手前请先读 [CLAUDE.md](CLAUDE.md)（语言边界与既定决策）、[代码规范](docs/06-代码规范与开发指南.md)与[架构守护规则](docs/05-架构与代码维护文档.md)——尤其注意：Go↔Rust 禁止 FFI、同机通信禁止 TCP 回环（ADR-011）、依赖白名单制。

## License

TBD（计划 Apache-2.0，开源时机见 PRD 开放问题 Q1）。
