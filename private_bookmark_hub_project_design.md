# 私人收藏中心：需求分析与完整项目设计

> 版本：1.0 · Passkey 认证整合版  
> 日期：2026-09-19  
> 暂定程序名：`bookmarkd`（仅作为工程和命令行名称，不限制最终产品命名）  
> 交付目标：一个自托管的私人收藏 Web 应用，发布 macOS、Linux、Windows 单文件可执行制品。  
> 文档状态：需求与设计基线；文中的接口、命令和数据结构是待实现的契约，不表示已经实现或通过实机测试。

## 阅读导航

| 范围 | 章节入口 |
|---|---|
| 基线与产品 | [0 依据与决策](#section-0) · [1 定位](#section-1) · [2 范围](#section-2) · [3 用户流程](#section-3) |
| 业务设计 | [4 页面](#section-4) · [5 收藏规则](#section-5) · [6 搜索](#section-6) · [7 收藏入口](#section-7) · [8 导入导出](#section-8) |
| Passkey 认证 | [9 模块架构](#section-9) · [10 配置与共享](#section-10) · [11 初始化与恢复](#section-11) · [12 登录与会话](#section-12) · [13 接口合同](#section-13) |
| 工程实现 | [14 应用架构](#section-14) · [15 数据库](#section-15) · [16 HTTP API](#section-16) · [17 配置与 CLI](#section-17) |
| 运行与交付 | [18 安全](#section-18) · [19 部署备份](#section-19) · [20 技术与制品](#section-20) · [21 非功能需求](#section-21) |
| 验收与依据 | [22 验收矩阵](#section-22) · [23 里程碑](#section-23) · [24 完成标准](#section-24) · [25 附件对照](#section-25) · [26 参考资料](#section-26) |

<a id="section-0"></a>

## 0. 文档依据与决策说明

### 0.1 依据

本设计以两部分内容为基础：

- 本次对话已经讨论的私人收藏中心需求：跨设备、跨浏览器、统一数据源、收藏管理、搜索、导入导出、自托管和单文件交付。
- 用户提供的《Passkey / WebAuthn 认证模块开发提示词》，上传文件名为 `passkey_webauthn_auth_module_prompt(1).md`。下文用 **[A1]** 表示该附件，用章节号定位来源。

**认证附件替代前一轮分析中的“强密码登录、密码重置、以后再增加 Passkey”方案。项目不保留任何正式密码登录或密码恢复路径。**

附件明确规定的模块边界、唯一正式登录方式、Setup 模式、共享凭据、独立 Session、SSH 恢复和安全要求均保留。文档中标注“本项目决策”或“本项目补充”的内容，是为了让宿主应用可以实际实现而补齐的行为，不宣称这些细节已经在附件中逐项规定。

官方技术资料仅用于核对标准和依赖边界，不取代附件的产品要求；引用编号见第 26 章。没有得到实机测试支持的兼容性和性能，均写成验收目标，而非已验证结论。

### 0.2 关键设计决策

| 事项 | 本版本决策 | 依据或性质 |
|---|---|---|
| 产品定位 | 单用户、自托管的私人收藏中心 | 对话需求 |
| 数据模型 | 一份服务端主数据，多个浏览器访问 | 对话需求 |
| 正式登录 | 仅 Passkey / WebAuthn | [A1] §3、§8 |
| 登录页 | 一个 Passkey 登录按钮，一个“在此设备保持登录 30 天”勾选项 | 用户本次要求；[A1] §8、§22 |
| 认证交付 | 嵌入宿主进程的可复用子模块，不是独立认证服务 | [A1] §1、§26 |
| 共享模型 | 共享 credential，宿主 Session 独立，不实现 SSO | [A1] §2、§9、§11 |
| 初次注册与恢复 | 管理终端 / SSH + 一次性 Setup Token + 浏览器注册 | [A1] §3、§6、§16 |
| 后端技术路线 | Rust；HTTP 适配层采用 Axum / Tokio | 本项目选型，延续上一轮建议 |
| 前端技术路线 | TypeScript + Svelte + Vite，构建为静态资源 | 本项目选型 |
| 默认存储 | 本地 `bookmarks.db` 与独立 `auth.db` | 本项目决策，认证存储保留接口抽象 |
| 运行形态 | 单进程、单业务实例，前后端资源嵌入可执行文件 | 对话需求与本项目决策 |
| 外部服务依赖 | 核心收藏功能不依赖 CDN、AI、邮件或外部数据库服务 | 本项目决策 |

### 0.3 对“单选框”的实现解释

用户描述的是一个可选择是否启用的“30 天保持登录”选项。界面只有这一项，不增加选项组。

**语义和 HTML 实现使用可以勾选、再次取消的 `checkbox`，而不是无法单独取消的 `radio`。**这只是控件语义的明确化，不改变“一个按钮＋一个保持登录选项”的界面要求。

### 0.4 默认值的性质

除附件明确规定或建议的值以外，本文的 12 小时短会话、5 分钟 Challenge、输入长度、分页、限流、导入大小和性能指标，都是本项目拟定的初始值。实现应集中定义、可测试，部分允许通过配置调整；不得把它们误认为 WebAuthn 标准规定。

---

<a id="section-1"></a>

## 1. 产品定位与目标

### 1.1 要解决的问题

用户有多个设备和多个浏览器。现有收藏通常依赖某个浏览器的账号和同步体系，换浏览器或临时使用其他设备时，无法直接访问完整的私人收藏。

本项目把收藏从浏览器中独立出来：

```text
任意可用设备上的受支持浏览器
        ↓
打开自己的收藏中心网址
        ↓
Passkey 验证，或使用尚未过期的本机浏览器会话
        ↓
搜索、浏览、添加和整理同一份私人收藏
        ↓
当前设备的浏览器直接打开目标网址
```

“任意浏览器”是跨浏览器产品目标，不是对不支持 WebAuthn 的历史浏览器的兼容承诺。登录仍要求浏览器、操作系统和认证器组合能够完成标准 WebAuthn 流程。[A1 §25]

### 1.2 核心价值

**摆脱特定浏览器绑定，减少同步复杂度，保证收藏快速可用、完整可迁移。**

系统不是浏览器本地收藏夹的双向同步工具，也不是目标网站的代理。浏览器只访问同一服务，不在每台设备运行独立服务并互相合并数据。

### 1.3 成功标准

用户应能把已有收藏迁入，在另一台设备登录、搜索并打开；新链接保存不依赖完整分类或抓取成功；数据不会因并发编辑、重复导入或升级而无提示丢失；随时可以导出和备份。

### 1.4 访问边界

访问收藏中心和访问收藏中的目标网站是两件事。内网地址能否打开，由当前设备的网络决定；收藏中心不替用户绕过网络限制，不同步目标站点的 Cookie、账号、密码或登录状态。

服务端拥有收藏明文。本设计提供访问控制和安全传输，不是让服务器也无法读取数据的端到端加密系统。

---

<a id="section-2"></a>

## 2. 范围与版本划分

### 2.1 V1 必须交付

| 模块 | V1 范围 |
|---|---|
| 认证 | Passkey 登录、首次 Setup、SSH 恢复、短会话、30 天会话、注销、会话撤销 |
| 认证复用 | 独立认证子模块、AuthStore 抽象、本机共享 credential、独立 Session、第二宿主测试 |
| 收藏 | 新增、查看、编辑、删除、恢复、网址复制、置顶、手动排序 |
| 分类 | 多层文件夹、标签、未分类收件箱、备注 |
| 搜索 | 标题、网址、域名、备注、标签；中文短词和英文大小写处理；目录筛选 |
| 添加入口 | 手动添加、粘贴网址、桌面 Bookmarklet、登录后恢复待添加内容 |
| 迁移 | Netscape Bookmark HTML 导入、预览与报告、重复处理；HTML 和完整业务 JSON 导入导出 |
| 管理 | 批量移动、批量加减标签、批量删除、回收站 |
| 多设备 | 页面重新获得焦点时检查更新、乐观并发控制、错误时保留编辑输入 |
| 界面 | 桌面和手机适配、日常浏览与整理模式区分、基本键盘可访问性 |
| 运维 | CLI、配置校验、数据库迁移、一致性备份与恢复、日志脱敏、健康检查 |
| 交付 | 前端内嵌、三平台制品、构建和运行说明、自动化与实机测试报告 |

### 2.2 V1.1 优先增强

快捷别名、批量粘贴多个网址、保存筛选条件、可选标题与图标补全、主屏幕安装体验、适配平台的系统分享入口、二维码传递普通收藏链接。

这些增强不应成为 V1 运行依赖。特别是图标、标题抓取和分享适配，必须能够独立关闭。

### 2.3 暂不实现

不实现多人注册、RBAC、团队协作、公开分享、完整 SSO、浏览器原生收藏双向同步、网页全文抓取、网页快照、AI 摘要或分类、向量检索、离线编辑、多业务副本同步、自建 Passkey 二维码协议、邮件找回、TOTP、密码或网页恢复码。

认证模块不依赖独立认证服务器；默认部署不要求 Docker、Redis、PostgreSQL 或 Node.js 运行时。以后可以通过适配器扩展认证存储，但不能把预留接口写成已经交付的数据库驱动。

---

<a id="section-3"></a>

## 3. 主要用户流程

### 3.1 首次部署

管理员在部署机器的终端初始化数据目录、配置域名和 RP 身份，明确启用首次 Setup；通过 CLI 生成一次性令牌；在支持 WebAuthn 的浏览器打开 `/setup`，验证令牌并注册 Passkey。注册完成后返回极简登录页，使用新 Passkey 登录，再导入现有收藏。

**Setup 成功不直接创建业务登录 Session。**用户随后完成一次正式 Passkey 登录，保持认证模型清晰。[本项目决策]

### 3.2 日常访问

打开首页后先检查当前会话。有效则直接显示收藏；无效则显示登录页面。点击 Passkey 按钮，由系统完成凭据选择与用户验证。成功后恢复原本准备访问的站内页面。

### 3.3 保存新链接

通过添加页面或 Bookmarklet 带入网址和标题。没有文件夹、标签或备注也能保存；默认进入收件箱。未登录时只保存当前标签页的临时待添加信息，认证成功后继续，不要求重复复制。

来自外部页面的链接**只预填，不自动保存**。用户必须在收藏中心显式确认。

### 3.4 集中整理

在整理模式批量选择条目，移动文件夹、增减标签、删除或恢复。系统明确展示本次影响范围。批量写入默认整体成功或整体失败，不出现无法解释的半完成状态。

### 3.5 换设备或更换认证器

只要新浏览器能够使用已注册 Passkey，或通过浏览器提供的跨设备认证方式使用原有凭据，就不需要重复初始化服务。需要注册全新凭据时，由管理员发起受保护的 Setup 流程。[A1 §7、§8、§16]

### 3.6 丢失全部可用 Passkey

通过 SSH 或部署机器的管理终端，显式开启受令牌保护的恢复注册流程，生成新的 Setup Token，再在浏览器注册新 Passkey。是否撤销旧凭据由管理员判断；仅仅暂时无法访问某台设备，不应自动删除所有旧凭据。

### 3.7 迁移服务器

备份业务数据和必要的认证资料，停止目标端写入后恢复，核对身份配置与权限。保留相同 RP ID、User ID 和服务端 credential 时，不应要求重新注册；但恢复后的旧 Session 和所有初始化令牌一律作废。[本项目补充]

---

<a id="section-4"></a>

## 4. 页面与交互设计

### 4.1 登录页：强约束

常态下仅有以下两个交互控件：

```text
[ 使用 Passkey 登录 ]

□ 在此设备保持登录 30 天
```

允许一个非交互的产品标题，以及执行中、失败或不支持状态的文字提示。不得增加用户名、密码、注册、忘记密码、TOTP、恢复码、其他登录方式或自定义二维码入口。[A1 §8、§22]

具体行为：

- 保持登录默认不勾选，不读取其他设备的偏好自动勾选。
- 点击按钮后进入等待状态，避免同时触发多次 `navigator.credentials.get()`。
- 用户取消认证时，恢复按钮，提示“已取消，请重新点击登录”；不把取消当作账户异常。
- 会话检查应先完成；已经登录的浏览器无需再弹 Passkey。
- 未初始化时显示非交互提示“服务尚未完成初始化，请通过服务器管理终端处理”，不公开 Setup 快捷入口。
- 浏览器不支持时显示说明，不降级成密码、链接令牌或公开访问。
- 不在页面加载时自动发起认证弹窗；不实现需要用户名输入框的条件式自动填充 UI。

浏览器或系统原生认证界面中出现的设备列表、PIN、指纹、人脸、手机扫码或安全密钥选择，不属于网站额外增加的登录控件。[A1 §8]

### 4.2 首页

顶部为全局搜索和新增入口；中间为手动排序的置顶收藏；下方显示最近新增或选中文件夹内容。桌面端可收起目录栏；移动端优先保证搜索、新增和条目点击区域。

默认是日常使用模式，不首先展示统计图或管理表单。完整收藏库采用紧凑列表，常用区可以采用紧凑卡片。条目标题直接打开目标网站，编辑使用独立操作入口。

### 4.3 页面清单

| 路径 | 页面 | 访问方式 |
|---|---|---|
| `/` | 首页与置顶导航 | 私人数据需 Session |
| `/library` | 全部收藏、目录、标签、搜索 | 私人数据需 Session |
| `/capture` | 外部带入或手动添加页面 | 可载入无数据页面壳；保存需 Session |
| `/trash` | 回收站 | Session |
| `/settings` | 界面偏好、数据导入导出、登录设备 | Session |
| `/settings/security` | 凭据只读信息、会话管理 | Session；敏感操作需近期 Passkey 验证 |
| `/login` | 极简登录页 | 公开，无私人数据 |
| `/setup` | 管理员初始化 / 恢复页 | 模式与令牌共同保护 |

为处理严格 SameSite Cookie 的外部导航，普通业务页可以返回不含私人数据的静态页面壳，再在本源脚本中检查 Session。**公开页面壳不等于公开收藏，也不得预嵌任何私人数据。**

### 4.4 添加和编辑

必填只有网址。未提供标题时暂用解析后的主机名作为展示标题；用户自填标题不得被后续补全覆盖。默认文件夹为收件箱。编辑失败、网络断开或版本冲突时保留当前输入。

### 4.5 打开方式

默认新标签页打开，可以设置为当前标签页。使用普通链接，避免依赖异步脚本打开而被浏览器拦截；新页面使用 `noopener`，并避免把收藏中心路径作为 Referrer 发送给目标站点。

复制链接不应修改最近访问顺序。V1 不记录详细浏览历史；“最近新增”按保存时间排序，置顶位置不因使用频次自动变化。

### 4.6 偏好

主题、显示密度、默认打开方式可存入业务偏好。展开的目录、滚动位置等临时界面状态保持在当前页面或标签页，不应为了同步偏好而上传完整浏览轨迹。

---

<a id="section-5"></a>

## 5. 收藏领域规则

### 5.1 收藏实体

一条收藏属于一个主文件夹，可以拥有多个标签、备注和独立置顶状态。相同 URL 可以存在多条记录；URL 不设全局唯一约束。

保存原始网址 `url_raw`；另存解析和比较字段，供展示与疑似重复检测使用。不能为比较方便改写用户原始地址。

### 5.2 文件夹

支持嵌套和手动排序；V1 深度上限拟定为 32 层。移动时禁止把目录移到自身或后代中。同级有效目录的规范化名称不允许重复；恢复遇到同名目录时提示处理，不静默合并。

“收件箱”是固定逻辑入口，对应 `folder_id = NULL`，不作为可以删除的实体目录。导入根目录是普通目录，与收件箱区分。

删除非空目录前，提供“连同内容移入回收站”或“把内容移至指定目录 / 收件箱”。默认不硬删除子内容。

### 5.3 标签与备注

标签支持一条收藏多个标签。标签规范化用于去重，显示名称保留原文。删除标签不会删除收藏。

备注为纯文本，不支持可执行 HTML。V1 不提供富文本、附件或笔记系统能力。

### 5.4 置顶与排序

置顶和文件夹互不排斥。收藏在自己的目录和首页置顶区都有入口，但仍为同一实体。

目录排序、目录内收藏排序、置顶排序分别维护。客户端提交完整受影响顺序及版本；服务端事务处理。初期可用有间隔整数位置，必要时在事务内重新编号，不需要复杂的分布式排序算法。

### 5.5 回收站

记录 `deleted_at`、删除操作批次和原目录。V1 不按时间自动永久清理，防止产生不可见的数据丢失；允许主动清空并二次确认。

恢复目录时可以连同该删除批次的后代一起恢复；恢复单条收藏而原目录不可用时，提示恢复到收件箱或其他目录。不能留下指向已删除父目录的不可见有效收藏。

### 5.6 网址与文本校验

| 字段 | 初始上限 / 规则 |
|---|---|
| URL | 16 KiB，绝对 `http` / `https` URL；保存时去除两端空白 |
| 标题 | 512 个 Unicode 字符 |
| 备注 | 16,384 个 Unicode 字符 |
| 文件夹名、标签名 | 128 个 Unicode 字符，不能为空 |
| 单条收藏标签数量 | 64 |
| 批量修改数量 | 一次最多 500 条 |

V1 禁止可执行或本地危险 scheme，包括 `javascript:`、`data:`、`file:`；不在网站中提供这些链接的执行能力。包含 `username:password@host` 的地址拒绝保存，并说明应移除嵌入式凭据。

允许保存内网域名和私有 IP 的普通 HTTP(S) 链接。**允许保存不等于允许服务端抓取。**

### 5.7 并发与幂等

每个可编辑实体有单调递增的 `version`。更新、移动、删除、恢复均携带期望版本；不匹配返回 `409 VERSION_CONFLICT`，附当前可公开给已登录用户的最新实体。

新建、批量修改和导入提交接受 `Idempotency-Key`。同一键与相同请求体重试返回原结果；同键不同内容拒绝。单用户不等于不会重复点击、网络重试或多设备同时操作。

---

<a id="section-6"></a>

## 6. 搜索设计

### 6.1 搜索范围

搜索标题、原始网址、主机名、备注和标签。文件夹用于筛选，并在结果中显示路径。默认全库搜索，切换“当前目录”时必须明确展示范围。

多个空格分隔的词采用 AND 语义；每个词可以命中不同字段。英文使用 Unicode 规范化和大小写折叠后的比较字段；中文支持连续子串，明确支持一两个汉字的查询。

原始数据不做破坏性大小写转换。`%`、`_` 等输入在普通搜索中按字面量处理，不作为 SQL 通配符或查询语法。

### 6.2 排序

建议优先级为：完整标题 / 主机名精确匹配、标题前缀、标题包含、域名或 URL 包含、标签和备注包含。相同分数采用稳定次级排序，不因刷新随机跳动。

首版没有拼音、语义、同义词或网页正文检索承诺。

### 6.3 技术实现

初期保存独立规范化检索字段，在 SQLite 中通过参数化查询、连接标签和包含匹配实现。不要仅启用一个 FTS 索引便宣称中文短词验收通过；FTS5 的 tokenizer 与短词行为需要额外测试。[R8]

默认分页 50 条，最大 200；输入防抖初始值 150 ms。数据增长后可以替换索引实现，但搜索语义保持不变。

### 6.4 更新感知

业务数据库维护 `library_revision`。页面重新获得焦点、重新联网、完成写入后检查版本。页面持续可见时可每 60 秒检查一次；后台标签页不轮询。

版本检查是轻量接口，不返回整个收藏库。发现变化后只更新受影响视图；存在未保存表单时不直接覆盖输入。

---

<a id="section-7"></a>

## 7. 收藏入口与外部带入

### 7.1 手动添加

始终保留不依赖扩展的添加入口。用户手动粘贴 URL，确认后保存。抓取失败、服务器不能访问目标站点或没有网络，都不应阻止保存一个语法有效的 URL。

### 7.2 Bookmarklet

为桌面浏览器提供“收藏到我的收藏中心”的书签小工具，仅带入当前地址与标题，不包含认证令牌、Session 或 Setup Token。

建议带入形式：

```text
https://bookmark.example.com/capture#url=<编码后的完整URL>&title=<编码后的标题>
```

片段内容由前端读取，不放在服务端查询参数中。前端读取后使用 `history.replaceState` 清理可见地址，把当前待添加草稿放进当前标签页的临时状态。

不得声称片段完全不留本地痕迹；浏览器历史和客户端环境仍需考虑。此设计主要减少服务端访问日志中出现私人目标 URL。

Bookmarklet 是便利功能，不是唯一入口；受浏览器策略或目标网页策略限制时，手动复制路径仍完整可用。

### 7.3 登录续接

待添加草稿只保存在当前标签页的 `sessionStorage`，初始 TTL 为 30 分钟，仅包含本次带入字段，不缓存完整收藏库或认证材料。

发现未登录时转入登录视图。认证完成后回到 `/capture` 恢复草稿。成功保存、主动取消或显式退出登录后清理草稿；会话自然到期导致重新登录时可以保留尚未保存的本次输入。

`return_to` 只允许经过解析和白名单校验的站内相对路径；拒绝绝对网址、协议相对网址、反斜杠变体、控制字符和反复编码的绕过，不成为开放重定向入口。

### 7.4 Strict Cookie 与外部导航

认证附件要求 `SameSite=Strict`。从外部页面进入 `/capture` 时，首次导航可能不携带该 Cookie。[A1 §12；R2]

因此不要把第一次导航请求“没有 Cookie”直接等同于“浏览器没有有效会话”。返回无私人数据的页面壳，由该页面执行本源 Session 检查，再决定是否显示登录视图。此行为必须在跨站 Bookmarklet 和普通外部链接场景中测试，不能通过降级 Cookie 策略绕过。

### 7.5 手机与后续扩展

V1 提供完整的手机网页。后续按实际平台增加主屏幕安装、PWA 分享或快捷指令入口，不承诺所有浏览器均支持相同的分享接收能力。[R9]

浏览器扩展和自动化 API 令牌不在 V1 范围。内部 HTTP API 只使用本项目 Session，不因为提供了 API 就引入第二套正式登录方式。

---

<a id="section-8"></a>

## 8. 导入、导出和数据迁移

### 8.1 HTML 导入

支持浏览器常用的 Netscape Bookmark HTML 结构；保留文件夹层级、标题、原始 URL，以及可可靠读取的原始时间。原始时间和“本次保存到系统的时间”分字段存放。

导入器只解析结构，不执行 JavaScript，不加载远端资源，不把上传文件直接作为同源 HTML 页面展示。

初始限制：单文件 20 MiB、最多 50,000 条收藏、目录深度 32。超限应在写入前拒绝，并提供可理解的原因。

字符编码先处理 BOM 和声明；无法确定或存在解码替换时展示警告，允许用户显式选择受支持编码，不默默把中文变成乱码后完成导入。

### 8.2 导入流程

```text
上传文件
→ 解析到暂存区
→ 展示数量、层级、异常与重复预览
→ 选择目标根目录、重复策略
→ 用户确认
→ 一个受保护的提交操作
→ 返回新增 / 跳过 / 失败报告
```

上传和预览不修改正式收藏。暂存内容与操作所属应用绑定，默认一小时过期，位于不可通过静态路由访问的临时目录。服务重启后将未完成的易失任务标记为过期或中断，不假装已经提交。

### 8.3 重复策略

默认只跳过明确的同目录、同标题、同比较地址记录，不自动合并跨目录或不同备注的收藏。相同 URL 可合法重复。

重复检测采用保守 URL 比较：规范化 scheme / host 和默认端口；不删除 query，不删除 fragment，不排序查询参数，不自动删追踪参数，不改变路径大小写。

对同一文件与同一导入目标维护文件摘要和策略版本，重复导入时明确提醒；通过幂等提交避免网络重试复制数据。用户可以明确选择保留重复，不能全局禁止。

### 8.4 导入报告

报告至少包括：解析条目、成功新增、跳过重复、无法识别、超限、禁止 scheme、编码警告、元字段丢弃情况。非法条目不执行，但应出现在报告中。不得静默丢弃 `javascript:` 书签或不支持的条目后声称完整迁移。

### 8.5 HTML 导出

用于回到浏览器，主要导出有效收藏的层级、标题和 URL。HTML 不一定承载全部应用专有字段，所以不能把它称为完整备份。

下载响应使用附件方式和转义后的内容。文件名不包含不可信路径或响应头字符。

### 8.6 JSON 导入导出

JSON 用于完整业务迁移，至少包含：格式版本、导出时间、文件夹、收藏、标签、关联、置顶与排序、备注、原始时间、业务偏好。可以显式选择包含回收站。

不包含认证凭据库、Session、Setup Token、CSRF 密钥、代理配置或服务器文件路径。JSON 恢复保留实体 ID 或按明确策略重新映射；引用不存在、ID 冲突或格式版本不支持时先报告，不部分覆盖现有数据。

示意结构：

```json
{
  "format": "bookmarkd-export",
  "format_version": 1,
  "exported_at": "2026-09-19T10:00:00Z",
  "includes_trash": false,
  "folders": [],
  "bookmarks": [],
  "tags": [],
  "bookmark_tags": [],
  "preferences": {}
}
```

导入和导出必须满足业务字段的往返测试。整机备份另见第 19 章。

---

<a id="section-9"></a>

## 9. 认证模块总体设计

> 本章至第 13 章整合 [A1] 的认证规范。项目级默认值和事务补充均单独说明。

### 9.1 模块边界

认证模块负责 WebAuthn 注册 / 登录、凭据、Setup Token、Challenge、Session、认证状态和管理 API。它不启动 HTTP Server，不持有收藏业务数据，不实现复杂用户系统，不强制其他宿主使用 Axum 或 SQLite。[A1 §1]

宿主负责 HTTP Server、路由、页面、业务跳转、配置注入、存储实现及访问保护。Axum 适配层是可选集成层，不是认证核心的依赖。

```text
bookmarkd 宿主进程
├── 业务模块：收藏、文件夹、搜索、导入导出
├── HTTP / 页面适配：Axum + 嵌入式静态前端
└── 认证子模块
    ├── Config
    ├── WebAuthnService / WebAuthnEngine
    ├── CredentialService
    ├── SetupService
    ├── SessionService
    ├── AuthStore / ChallengeStore
    └── 可选 HTTP 适配层
```

### 9.2 唯一正式认证链路

```text
服务器管理终端 / SSH 权限
→ 一次性 Setup Token
→ 浏览器注册 Passkey
→ Passkey 登录
→ 宿主独立 Session
→ 私人业务资源
```

Setup Token 只能授予注册权限，不能直接读取收藏、签发普通 Session 或作为 API Key 使用。[A1 §3；本项目权限细化]

Windows 本机终端和 macOS 本机终端是同样的管理入口，不要求机器必须运行 SSH 服务；远程恢复可以通过 SSH。管理终端权限仍是最高信任边界。

### 9.3 对多凭据的支持

单用户可以注册多个 Passkey，名称如“手机”“电脑”“备用安全密钥”。服务端存储公钥和关联元数据，不存储客户端私钥。[A1 §13]

V1 通过 CLI 创建 Setup 授权来添加新凭据。登录后的安全页面可以查看凭据、修改名称；不默认提供一个绕开 Setup 模式的注册入口。后续若支持网页内增加凭据，必须单独设计近期 Passkey 再认证的授权流程，不能凭长期 Session 无条件注册。

### 9.4 模块与存储的信任关系

共享可写 AuthStore 的宿主进程属于同一认证信任域。一个被攻陷且拥有认证库写权限的宿主，可以破坏共享认证状态；“不同 Session”不意味着它们在数据库写权限层面互相隔离。[本项目威胁模型]

因此只允许同一管理员控制的服务共享认证存储，不向不受信任的第三方应用开放认证库写权限。

---

<a id="section-10"></a>

## 10. 认证配置与跨子域复用

### 10.1 配置定义

| 字段 | 含义 | 本项目默认 / 规则 |
|---|---|---|
| `rp_id` | WebAuthn RP ID | 显式配置；共享示例 `example.com` |
| `rp_name` | 系统认证界面显示名称 | `Private Services`，可配置 |
| `user_id` | 稳定二进制 User Handle | 首次初始化生成 32 字节 CSPRNG 值，或加载已有固定值 |
| `user_id_file` | User ID 输入文件 | 与直接值互斥；Base64url 无填充编码 |
| `user_name` | 只用于展示的名称 | `owner` |
| `display_name` | 用户显示名称 | 默认同 `user_name` |
| `allowed_origins` | 精确 Origin 白名单 | 默认仅本应用公开 Origin |
| `app_id` | 宿主应用稳定标识 | `bookmarkd`；用于会话、令牌和审计作用域 |
| `public_url` | 对外标准地址 | HTTPS；不从任意 Host / Forwarded 头推断 |
| `setup_mode` | Setup 开关 | 正常运行默认 `disabled` |
| `setup_token_ttl` | 一次性注册授权有效期 | 10 分钟 |
| `challenge_ttl` | 注册 / 登录挑战有效期 | 5 分钟，不超过所属 Setup 授权剩余期限 |
| `session_short_ttl` | 未勾选保持登录 | 12 小时绝对有效期 |
| `session_long_ttl` | 勾选保持登录 | 固定 30 天，即 2,592,000 秒 |
| `recent_auth_ttl` | 敏感操作的近期验证窗口 | 5 分钟 |
| `auth_store` | 认证存储适配器与连接信息 | 默认本地 SQLite 路径 |

RP ID、RP Name、User ID / User ID 文件、User Name 都支持宿主 CLI 参数传入，另外必须支持 Origin、应用身份、存储和 Setup 模式。不能误认为只有四个身份值就足以完整配置认证。[A1 §4、§9]

### 10.2 身份持久性

`user_id` 推荐按附件生成 256 bit 随机值；为了接入已有认证体系，允许符合模块策略的既有 16～64 字节稳定 User Handle，必须逐字节一致。不得基于 hostname、日期或显示用户名重新生成。[A1 §4.3；本项目编码补充]

已有有效凭据时，改变 RP ID 或 User ID 必须启动失败并报告身份配置冲突。不得静默创建一个新身份；不得为了某个库只接受 UUID 而截断、哈希替换或重生成已使用的 User ID。

显示名称可以通过显式配置变更，但不是主键。首次初始化应是独立、明确的动作，服务启动时不得因为路径写错而悄悄新建空认证库。

### 10.3 共享示例

```text
收藏服务：bookmark.example.com
笔记服务：obsidian.example.com
工具服务：tools.example.com

RP ID：example.com
RP Name：Private Services
User ID：同一组固定字节
User Name：owner
Credential：访问同一份有效服务端凭据数据
Session：按宿主 app_id 与 Origin 独立
```

第二个服务已能读取有效 credential、身份一致、Setup 为 `disabled` 或 `auto` 时，直接提供 Passkey 登录，不重新注册。[A1 §9、§11]

复制一次数据库或 JSON 文件不能被描述成长期共享：之后新增、撤销和 counter 更新不会自动一致。正式共享需要所有宿主访问同一权威存储。

### 10.4 Origin 白名单

RP ID 不含 scheme、端口和路径；Origin 是 scheme、host、有效端口构成的完整来源。共享 RP ID 不等于接受任意子域 Origin。[A1 §14；R1]

默认每个宿主仅允许自己的公开 Origin。一个宿主确需多个合法入口时逐项列出，并在 Challenge 中绑定本次实际 Origin。禁止通配所有子域、任意端口，以及按不可信请求头自动扩充白名单。

同一 RP 下的所有可注册来源都需谨慎维护，不将第三方可控内容、遗留不受管子域纳入认证信任范围。

### 10.5 Session 与 Cookie 隔离

默认 Cookie 名 `__Host-bookmarkd_session`，`Path=/`，不设置 `Domain`。其他宿主使用自己的 Cookie 名，同时在服务端检查 `app_id` 和创建时绑定的 Origin。[本项目补充；R2]

即使手动把 A 服务的 Cookie 复制给 B，B 也不能将其当作自己的有效 Session。不同端口不是充分的 Cookie 隔离边界，推荐不同宿主使用不同子域。

共享的含义是“同一 Passkey 能分别登录”，不是“A 已登录，B 自动登录”。[A1 §2、§11]

### 10.6 默认共享存储

本机多个可信进程可以指向同一本地 `auth.db`，由适配器处理事务和锁；业务数据库仍分别独立。不同服务器不得通过 NFS / SMB 并发访问同一 SQLite 文件。[A1 §21；R3]

跨服务器共享时，通过 AuthStore 适配远程数据库或其他权威存储。V1 交付 SQLite 实现和接口合同，不把 PostgreSQL / MySQL / Redis 驱动列为默认必交付代码。

---

<a id="section-11"></a>

## 11. Setup、注册与恢复

### 11.1 Setup 模式状态表

| 模式 | 有效凭据数量 | 有效令牌 / 注册授权 | 是否可注册 |
|---|---:|---|---|
| `disabled` | 任意 | 任意 | 否；`/setup` 返回不可用 |
| `auto` | 0 | 有 | 是 |
| `auto` | 0 | 无 | 否，必须先从 CLI 生成令牌 |
| `auto` | ≥1 | 任意 | 否 |
| `enabled` | 任意 | 有 | 是，仅限令牌对应注册操作 |
| `enabled` | 任意 | 无 | 否 |

**所有注册阶段都重新检查模式和存储状态，不只在页面打开时检查。**`enabled` 绝不是匿名注册开关。[A1 §5；本项目并发补充]

`auto` 注册第一个凭据后逻辑上关闭 Setup，不要求程序改写配置文件。`enabled` 的授权用完后关闭本次流程；没有未过期授权时 `/setup` 不可用。生产环境仍建议显式改回 `disabled`。

### 11.2 Setup Token

使用 CSPRNG 生成至少 32 随机字节，Base64url 编码；默认 10 分钟，一次注册使用一次。随机令牌、Session 令牌和 CSRF 数据采用不同类型，不能混用。

令牌在显式 CLI 命令执行时仅输出一次到管理员终端。普通服务启动、应用日志、HTTP access log、异常堆栈和审计事件均不得包含完整令牌。令牌不进入 URL 查询参数或路径。[A1 §6]

附件优先建议内存；但 CLI 是另一个进程，纯内存令牌不能天然被运行中的宿主看到。**本项目选择附件允许的持久化分支：默认只在 `auth.db` 保存带用途区分的 SHA-256 摘要，不保存明文。**这样无需另建管理 HTTP 端口或依赖独立常驻认证进程。[A1 §6；本项目决策]

新建授权默认撤销同应用旧的未完成 Setup 授权，避免遗留多个有效入口。敏感比较采用常量时间方法；消费、过期或撤销后删除令牌摘要，只保留不含令牌的审计事件。

### 11.3 Token 与浏览器绑定

```text
issued
→ verified_and_bound
→ consumed（注册事务成功，摘要删除）

issued / verified_and_bound
→ expired 或 revoked（摘要删除）
```

用户在 `/setup` 输入 Token，通过 POST 请求体提交。服务端验证后，将注册授权原子绑定到该浏览器的随机预认证 Cookie、宿主和 Origin。

验证令牌本身不创建业务 Session。已绑定的授权不能被另一浏览器抢用。浏览器取消创建凭据后，可以在原授权有效期内发起新的注册 Challenge；不延长原 Token 的绝对有效期。

最后一步保存 credential 和消费 Token 必须在同一认证存储事务中完成。两个并发完成请求最多成功一个；`auto` 模式下跨进程竞争第一个凭据也必须按同一规则串行检查。

### 11.4 注册策略

由成熟 WebAuthn 库生成并验证注册状态，前端只调用标准 `navigator.credentials.create()`。[A1 §7、§14]

| 参数 | 本项目目标 |
|---|---|
| RP ID / Name | 从固定配置加载 |
| User ID | 稳定二进制值，不由前端决定 |
| User Name / Display Name | 展示字段 |
| `challenge` | 每次新生成，服务端保留配对状态 |
| `userVerification` | `required`，服务端验证 UV |
| `residentKey` | `required`，选择附件允许的 discoverable 策略 |
| `requireResidentKey` | 与 required 策略一致的兼容字段，由库处理 |
| `authenticatorAttachment` | 不限定为仅本机或仅外接 |
| `attestation` | `none`，不要求特定厂商背书；仍须完成结构与协议验证 |
| `excludeCredentials` | 当前身份已有的有效凭据 |

`residentKey=required` 会排除不能创建可发现凭据的认证器，这是本项目为了新凭据具备可发现能力作出的兼容性取舍，不宣称所有 FIDO 安全密钥都满足要求。无用户名界面本身并不要求所有既有凭据都可发现，既有凭据登录兼容见第 12.1 节。[R1]

库选型必须支持以上要求。不能简单依赖库名里出现 Passkey 就假设默认参数一致；具体依赖约束见第 20.3 节。

### 11.5 注册时序

```text
管理员 CLI：创建 Token（存摘要）
浏览器：GET /setup
浏览器 → POST /auth/setup/verify：提交 Token
服务端：验证、绑定浏览器，返回可注册状态
浏览器 → POST /auth/register/begin
服务端：核对模式和授权，生成 options，保存配对注册状态
浏览器：navigator.credentials.create()
浏览器 → POST /auth/register/finish：提交 credential 响应
服务端：校验绑定、Origin、RP、Challenge、UV 与库验证结果
服务端事务：再次核对授权和模式，保存 credential，消费 Token
浏览器：清理注册状态，提示成功，返回 /login
```

私钥在认证器 / Passkey Provider 中产生并管理，服务端不能代生成客户端私钥。[A1 §7]

### 11.6 SSH 恢复的完整操作语义

1. 登录服务器管理终端，确认操作的是正确认证库和 RP 身份。
2. 当前为 `disabled` 时，显式将运行配置改为 `enabled` 并重启宿主，使变更生效。
3. 必要时撤销丢失或疑似泄露的 credential；凭据撤销连带影响的 Session 见第 12.6 节。
4. 执行 `auth create-setup-token`，浏览器访问 `/setup`，输入令牌并注册。
5. 使用新 Passkey 正式登录验证。
6. 把生产配置改回 `disabled` 并重启；核对会话和安全审计。

**生成 Token 不会自动改变运行中的 Setup 模式。**CLI 可生成一个尚不可用的短时授权并明确提示当前关闭状态，但 HTTP 注册必须始终服从实际运行配置。不要通过“只要有 Token 就忽略 disabled”实现恢复。[本项目补充]

若管理员保留了 `auto`，且删除了全部凭据，仍然需要有效 Setup Token；空数据库永远不等于匿名可注册。

---

<a id="section-12"></a>

## 12. 登录、Challenge 与 Session

### 12.1 登录流程

```text
浏览器载入无私人数据登录页面
→ POST /auth/login/begin，提交 remember_30d
→ 服务端生成登录 Challenge，记录本次期限选择和浏览器绑定
→ 浏览器调用 navigator.credentials.get()
→ 系统选择可用 Passkey / 手机 / 安全密钥并验证用户
→ POST /auth/login/finish
→ 服务端验证断言、有效凭据及一次性状态
→ 原子更新认证状态并创建本宿主 Session
→ 设置安全 Cookie
→ 回到合法站内目的页面
```

`remember_30d` 在 begin 阶段与服务端 Challenge 绑定。finish 阶段不能由客户端再改成更长的任意 TTL。

**本项目登录请求策略：单用户、无用户名输入，不把“无用户名界面”误等同于只能使用空 `allowCredentials`。**默认服务端从固定用户的有效凭据生成 `allowCredentials`，浏览器仍通过同一个 Passkey 按钮选择认证器。这样既能使用新注册的可发现凭据，也不会仅因其他宿主曾采用附件允许的 `residentKey=preferred`，就要求其中已存在的有效、支持 UV 的非可发现凭据重新注册。[A1 §7、§11；R1]

新注册仍采用第 11 章的 `residentKey=required`。适配器可另外支持省略或置空 `allowCredentials` 的真正可发现登录，但不能把它当作复用已有凭据的唯一途径。两种方式都不增加用户名输入控件；不得为了兼容而降低 UV 要求。

服务端必须将返回的 credential ID 映射到本次固定单用户的有效凭据。若响应带有 userHandle，必须逐字节验证；可发现登录必须取得并验证相应用户关联。已知凭据列表模式下允许协议规定的空 userHandle，但必须通过服务端凭据归属确定用户。与已知身份不匹配时拒绝，不自动创建新用户。登录 options 中必要的凭据标识属于协议数据，不能借此返回公开的管理列表、凭据名称或其他私人资料。[R1]

### 12.2 前端职责

前端处理标准 options / response 的二进制字段与 Base64url 编解码；优先使用通过测试的转换工具，不把 JavaScript 对象直接序列化导致 `ArrayBuffer` 变成空对象。

二维码、跨设备发现、BLE、CTAP / FIDO 通信由浏览器和操作系统负责，网站不实现自定义登录二维码、轮询扫码票据或厂商登录协议。[A1 §8、§25]

### 12.3 Challenge 存储

默认内存存储，但抽象为 `ChallengeStore`。存完整库配对状态，而不只是 challenge 字符串；这些状态只保存在服务端。[A1 §19；R4]

每条状态包含：操作 ID、类型、配对库状态、创建与过期时间、浏览器绑定摘要、`app_id`、Origin、RP / User 身份版本、相关 Setup 授权或 Session、保持登录选择。

类型至少区分 `registration`、`authentication`、`reauthentication`。任何一种响应不能用于另一种操作。

验证浏览器和作用域后，通过原子 take / consume 取得状态；同一操作最多交给验证器一次。验证失败也不能再次提交相同状态，需要新 begin。未授权浏览器不能仅凭猜测操作 ID 消费别人的 Challenge。

服务重启使内存中的未完成 Challenge 失效，前端重新开始，已持久化 Session 不因此丢失。未来多实例需要共享可原子消费的 ChallengeStore，不能仅使用每实例内存然后随机负载均衡。[A1 §19、§20]

### 12.4 Session 生命周期

| 登录选择 | Cookie | 服务端有效期 | 是否自动滑动续期 |
|---|---|---|---|
| 未勾选 | 浏览器会话 Cookie，不设置长期 Max-Age | 创建后 12 小时绝对到期 | 否 |
| 勾选 30 天 | `Max-Age=2592000`，可同时设置相应 Expires | 创建后 30 天绝对到期 | 否 |

30 天是最长保持时间，不保证浏览器不会提前清理 Cookie，也不保证被管理员撤销后仍可用。浏览器可能恢复会话 Cookie，所以不能声称关闭浏览器一定立即失效；服务端截止时间是最终约束。[A1 §12；R2]

会话过期必须再次使用 Passkey。更新 `last_seen_at` 不修改 `expires_at`。更改机器 IP、使用移动网络或切换代理不自动注销，IP 只可用于可选审计，不作为会话认证因子。

### 12.5 Session Token 与 Cookie

Session Token 使用 CSPRNG 生成至少 32 字节。服务端只保存摘要；Token 只通过 HttpOnly Cookie 传输，不出现在 JSON、网址、localStorage、日志或数据库明文字段。[A1 §12]

示例：

```http
Set-Cookie: __Host-bookmarkd_session=<opaque-token>; Path=/; Secure; HttpOnly; SameSite=Strict; Max-Age=2592000
Cache-Control: no-store
```

正式环境不设置 `Domain`。认证成功重新生成 Session，防止复用预认证身份。开发环境仅允许显式启用的 localhost 特例，使用与生产区分的 Cookie 和数据目录，不允许以“开发模式”在公网关闭保护。

### 12.6 撤销语义

Session 保存发起登录的 credential 记录 ID。凭据撤销后，删除或撤销共享存储中由该凭据建立的所有宿主 Session；全凭据撤销则撤销该认证身份的所有 Session 和未完成注册授权。[本项目安全补充]

每次业务请求都检查 Session 有效期、撤销状态、作用域及凭据仍有效。可以节流 `last_seen_at` 写入，但不能为提高性能把认证结果无限缓存。V1 目标是撤销完成后新请求立即失效。

`revoke-all-sessions` 默认只影响当前 `app_id`；明确指定 `--all-apps` 才影响整个共享认证身份，并输出影响范围。不能把“注销本服务”实现成未经说明的全站退出。

### 12.7 近期 Passkey 再认证

普通浏览、搜索、增改收藏不要求重复验证。查看安全页面不增加额外登录控件。

对清空回收站、完整导出、撤销其他 Session 等敏感操作，本项目补充“最近 5 分钟内通过 Passkey 验证”的要求。当前验证过旧时，在对应操作页面发起一次再认证；成功更新 `reauthenticated_at`，不自动延长 Session 的原始到期时间。

再认证 Challenge 绑定当前 Session 与用途，不能拿普通注册或其他宿主响应代替。V1 不通过再认证接口直接签发全新的长期会话。

### 12.8 Sign Counter

保存并更新计数器和库需要的同步 / 备份状态元数据。始终返回 0 的认证器不能仅因此认证失败；有效计数器回退记录安全事件，按所选库的验证结果和经测试策略处理，不自行构造不兼容的绝对规则。[A1 §24]

共享存储更新采用版本检查 / 事务，避免两个宿主并发验证后旧值覆盖新值。发现版本变化时需要重新读取并按库语义检查，不能简单把旧验证状态无条件写回。

### 12.9 安全失败原则

AuthStore 不可用返回服务暂不可用，不回退匿名。没有有效凭据不放开业务资源。不支持 Passkey 不提供密码后门。泄露一个 Session 不应自动允许添加一个永久认证器。

---

<a id="section-13"></a>

## 13. 认证接口合同

### 13.1 核心服务

| 服务 | 职责 |
|---|---|
| `WebAuthnService` | Begin / Finish Registration、Authentication、Reauthentication |
| `CredentialService` | List、Get、Rename、Revoke、RevokeAll |
| `SetupService` | CreateToken、ValidateAndBindToken、ConsumeToken、IsSetupAllowed |
| `SessionService` | Create、Validate、List、Revoke、RevokeAll |
| `AuthStore` | 凭据、身份、授权和会话持久化及原子事务 |
| `ChallengeStore` | 有界、带 TTL、可原子消费的临时认证状态 |
| HTTP Adapter | 提取请求、错误映射、Cookie、RequireAuth、近期认证检查 |

认证核心提供服务函数，不自带强制 CLI parser，不自行注册路由。[A1 §15、§17、§18]

### 13.2 AuthStore 必须实现的能力

```text
LoadIdentity / CreateIdentityIfEmpty / ValidateIdentity
ListCredentials / GetCredentialByID
SaveCredential / UpdateCredential / RenameCredential
DeleteCredential / DeleteAllCredentials
CreateSession / GetSession / ListSessions
DeleteSession / DeleteAllSessions(scope)
SaveSetupTokenHash / GetSetupAuthorization / DeleteSetupToken
BindSetupAuthorizationAtomically
CommitRegistrationAtomically
CommitAuthenticationAtomically
RevokeCredentialAndDependentSessionsAtomically
AppendSecurityEvent
```

附件中的 Delete / Revoke 对外表示凭据不再有效。存储内部可以保留必要墓碑或审计 ID，但不得继续把它作为有效 credential 返回。[A1 §10、§15；本项目实现语义]

不能只提供若干 CRUD 再靠调用顺序假装具备原子性。每个适配器必须通过同一组契约测试，证明一次性注册授权、凭据撤销、计数器更新与 Session 创建的竞态处理正确。

### 13.3 原子操作

**CommitRegistration：**事务内重新核对身份、Setup 模式对应条件、授权未过期且绑定一致、凭据不重复，再插入 credential 并删除令牌摘要。失败全部回滚。

**CommitAuthentication：**事务内确认凭据仍有效且版本符合验证前提，更新库状态，创建作用域正确的 Session。不存在“刚撤销但仍凭旧快照新建 Session”的窗口。

**RevokeCredential：**撤销 credential 和关联 Session 在同一权威认证存储事务中处理；并更新撤销代次，使未完成的相关认证流程不能越过撤销。

### 13.4 类型边界示意

以下为语言无关契约的 Rust 风格说明，不是某个第三方库可直接复制编译的 API：

```rust
struct AuthConfig {
    rp_id: String,
    rp_name: String,
    user_id: Vec<u8>,
    user_name: String,
    display_name: String,
    allowed_origins: Vec<Origin>,
    app_id: String,
    setup_mode: SetupMode,
    short_ttl: Duration,
    long_ttl: Duration,
    challenge_ttl: Duration,
    setup_token_ttl: Duration,
}

enum SetupMode { Disabled, Auto, Enabled }
enum CeremonyKind { Registration, Authentication, Reauthentication }

struct RequestScope {
    app_id: AppId,
    origin: Origin,
    browser_binding_hash: [u8; 32],
}

struct AuthContext {
    identity_id: IdentityId,
    session_id: SessionId,
    credential_id: CredentialRecordId,
    app_id: AppId,
    origin: Origin,
    authenticated_at: Timestamp,
    reauthenticated_at: Option<Timestamp>,
    expires_at: Timestamp,
}

struct PendingCeremony<S> {
    id: CeremonyId,
    kind: CeremonyKind,
    scope: RequestScope,
    expires_at: Timestamp,
    remember_30d: bool,
    identity_version: u64,
    state: S, // 所选成熟 WebAuthn 库的服务端配对状态
}
```

令牌明文类型禁止派生会输出内容的 `Debug`。业务代码只接收 `AuthContext`，不能接触 Session Token、Setup Token 或原始验证材料。

### 13.5 第二宿主验收夹具

仓库提供一个极小的示例宿主，仅返回受保护的测试页面，用第二个 `app_id` 和第二个合法 Origin 挂载相同认证模块。它只用于复用和跨服务测试，不是正式运行依赖，不随收藏服务额外启动独立认证服务器。

---
<a id="section-14"></a>

## 14. 应用架构与工程结构

### 14.1 运行架构

```text
浏览器
  │ HTTPS：页面、业务 API、标准 WebAuthn
  ▼
反向代理（可选部署组件，例如现有 HTTPS 网关）
  │ 受信任的本机连接
  ▼
bookmarkd 单进程
  ├── 静态资源服务：编译时内嵌
  ├── HTTP 适配：认证、CSRF、限流、错误映射
  ├── 收藏应用服务：查询、编辑、搜索、导入导出
  ├── 嵌入式认证模块：WebAuthn、Setup、Session
  ├── 有界任务执行：导入解析、备份、清理
  ├── bookmarks.db：本地业务数据库
  └── auth.db：本地默认认证数据库，可由同机可信宿主共享
```

数据库和文件目录不注册为静态目录；静态资源仅从编译产物中读取。业务模块调用 Repository，认证模块调用 AuthStore，两者不直接操作对方的数据表。

### 14.2 线程与事务

HTTP 使用异步运行时。SQLite 同步调用放入专用数据库工作线程或有界阻塞执行器，不在异步工作线程上长时间阻塞。

业务写入按事务提交；认证数据库可能有其他宿主和 CLI 写入，必须依赖数据库事务与版本校验，而不是仅依赖本进程互斥锁。

文件解析、JSON 导出和备份使用有界任务，防止一次大导入让登录请求无法响应。V1 单实例运行，不为未来扩展提前部署消息队列。

### 14.3 工程结构

```text
bookmarkd/
├── Cargo.toml
├── Cargo.lock
├── rust-toolchain.toml
├── crates/
│   ├── bookmark-domain/          # 业务实体、校验与不变量
│   ├── bookmark-service/         # 收藏用例、搜索、导入导出
│   ├── bookmark-store-sqlite/    # 业务持久化与迁移
│   ├── passkey-auth-core/        # 可复用认证服务与接口，不依赖 Axum
│   ├── passkey-auth-webauthn/    # 成熟 WebAuthn 库适配器
│   ├── passkey-auth-sqlite/      # 默认 AuthStore 与事务
│   ├── passkey-auth-axum/        # 可选 HTTP 适配器
│   └── bookmarkd/               # 主程序、CLI、配置、路由、内嵌资源
├── web/
│   ├── package.json
│   ├── package-lock.json
│   ├── vite.config.ts
│   ├── src/
│   │   ├── pages/               # Login、Setup、Home、Library、Capture、Settings
│   │   ├── components/          # Search、BookmarkList、FolderTree、TagPicker
│   │   ├── api/                 # 同源请求、类型、错误处理
│   │   ├── auth/                # WebAuthn 编解码、会话状态，不保存令牌
│   │   └── state/               # 当前视图、编辑草稿
│   └── public/                  # 本地图标，无远程字体依赖
├── migrations/
│   ├── business/
│   └── auth/
├── examples/
│   └── second-host/             # 模块复用测试夹具
├── tests/
│   ├── unit/
│   ├── integration/
│   ├── auth-contract/
│   ├── e2e/
│   └── fixtures/                # 各类导入文件、异常输入
├── scripts/                     # 构建、依赖审查和制品检查
└── docs/
    ├── design.md
    ├── auth-integration.md
    ├── operations.md
    └── compatibility-report.md
```

这是职责边界设计，不要求为了形式无限拆包。模块数量可以在不破坏依赖方向的前提下收敛，但认证核心必须能被另一个宿主直接依赖。

---

<a id="section-15"></a>

## 15. 数据库设计

### 15.1 通用约定

业务实体 ID 使用随机 UUID 文本；WebAuthn credential ID 和 User ID 使用二进制，绝不混用业务 UUID。数据库时间使用 UTC Unix 毫秒整数，API 使用带时区的 RFC 3339 字符串。

可编辑实体从 `version=1` 开始递增。逻辑布尔值约束为 0 / 1，枚举值通过 CHECK 或服务层验证。JSON 字段仅用于明确的可扩展元信息，不把整个业务库塞进一列。

每个连接启用外键约束。默认 WAL、`synchronous=FULL`、有界 busy timeout（初始 5 秒）。这些是实现初始策略，需在目标平台压测。[本项目决策；WAL 部署限制见 R3]

### 15.2 业务数据库

#### `folders`

| 字段 | 类型 | 说明 |
|---|---|---|
| `id` | TEXT PK | 实体 ID |
| `parent_id` | TEXT NULL FK | 上级目录，NULL 为普通顶层 |
| `name` / `name_norm` | TEXT | 展示名 / 规范化名称 |
| `position` | INTEGER | 同级手动顺序 |
| `version` | INTEGER | 乐观锁 |
| `created_at` / `updated_at` | INTEGER | 创建和变更时间 |
| `deleted_at` | INTEGER NULL | 回收站标记 |
| `delete_batch_id` | TEXT NULL | 批次恢复关联 |

#### `bookmarks`

| 字段 | 类型 | 说明 |
|---|---|---|
| `id` | TEXT PK | 收藏 ID |
| `folder_id` | TEXT NULL FK | NULL 为收件箱 |
| `title` | TEXT | 展示标题 |
| `url_raw` | TEXT | 原始可打开网址 |
| `url_compare` | TEXT | 保守比较地址，不设唯一约束 |
| `host` | TEXT | 解析后的域名或 IP |
| `note` | TEXT | 纯文本备注，默认空字符串 |
| `title_norm` / `url_norm` / `note_norm` | TEXT | 检索字段 |
| `position` | INTEGER | 当前目录中的手动顺序 |
| `is_pinned` / `pin_position` | INTEGER | 置顶状态与顺序 |
| `source_created_at` | INTEGER NULL | 导入来源的原始创建时间 |
| `created_at` / `updated_at` | INTEGER | 本系统保存与变更时间 |
| `version` | INTEGER | 并发版本 |
| `deleted_at` / `delete_batch_id` | INTEGER NULL / TEXT NULL | 回收站信息 |
| `import_batch_id` | TEXT NULL | 来源导入批次 |

V1.1 的 alias 或 metadata 字段通过独立迁移增加；不要求 V1 前端出现空占位功能。

#### 其他业务表

| 表 | 主要字段 | 约束与用途 |
|---|---|---|
| `tags` | id、name、name_norm、version、created_at | name_norm 唯一 |
| `bookmark_tags` | bookmark_id、tag_id | 复合主键，两端 FK |
| `preferences` | key、value_json、version、updated_at | 只保存业务 / 界面偏好 |
| `library_state` | singleton_id、revision | 每次业务事务递增 revision |
| `import_batches` | id、file_hash、target_folder_id、policy、status、counts_json、created_at | 记录导入结果，不保存无限期原文件 |
| `idempotency_records` | app_scope、key、request_hash、response_json、expires_at | 同作用域 key 唯一；初始保留 24 小时 |
| `schema_migrations` | version、checksum、applied_at | 独立业务迁移记录 |

### 15.3 业务索引与事务要求

至少索引有效收藏的目录、置顶顺序、更新时间；比较 URL 索引用于重复提示；标签关联有反向索引；目录名称唯一性针对有效记录和同一父目录。

SQL 中 NULL 父目录的唯一性要明确处理，例如以规范化 parent key 建立部分唯一索引，不能误以为普通 `(parent_id, name_norm)` 唯一索引会自动禁止所有顶层重名。

收藏正文、标签关联、版本和 `library_revision` 在同一事务更新。不能先保存收藏再异步保存标签，导致用户看到短暂不一致或失败后的残缺状态。

### 15.4 认证数据库

认证库由模块管理，不含书签、文件夹、浏览历史或业务配置。[A1 §1、§21]

#### `auth_identity`

| 字段 | 类型 | 说明 |
|---|---|---|
| `id` | TEXT PK | 认证身份记录 ID |
| `rp_id` | TEXT | 固定 RP ID |
| `user_id` | BLOB | 固定 User Handle |
| `rp_name` / `user_name` / `display_name` | TEXT | 展示配置 |
| `identity_version` | INTEGER | 身份配置代次 |
| `revocation_epoch` | INTEGER | 撤销相关的竞态保护代次 |
| `created_at` | INTEGER | 创建时间 |

默认实现只允许一个认证身份，不引入多用户系统。ID 只是跨表稳定引用，不是第二个用户名。

#### `webauthn_credentials`

| 字段 | 类型 | 说明 |
|---|---|---|
| `id` | TEXT PK | 管理界面 / CLI 使用的记录 ID |
| `identity_id` | TEXT FK | 所属单用户身份 |
| `credential_id` | BLOB UNIQUE | WebAuthn credential ID |
| `credential_state` | BLOB / TEXT | 经过版本化序列化的库凭据状态，包含公钥 |
| `state_format` / `state_version` | TEXT / INTEGER | 适配器格式与版本 |
| `sign_count` | INTEGER | 管理和并发检查字段，与权威库状态事务一致 |
| `transports_json` | TEXT | transport 元信息 |
| `backup_eligible` / `backup_state` | INTEGER NULL | 所选库验证并提供时保存 |
| `name` | TEXT | 用户可读名称 |
| `created_at` / `last_used_at` | INTEGER / INTEGER NULL | 时间信息 |
| `version` | INTEGER | 并发版本 |
| `revoked_at` | INTEGER NULL | 撤销标记 / 墓碑 |

公钥保存在库凭据结构内；若额外导出 COSE 公钥列，必须由适配器以成熟库接口完成，不能出现两套互相不一致的权威状态。库版本升级必须有该格式的迁移和往返测试。

凭据命名是用户标签，不能根据 User-Agent 断言“这把同步 Passkey 只属于某一台设备”。

#### `sessions`

| 字段 | 类型 | 说明 |
|---|---|---|
| `id` | TEXT PK | 非令牌的管理 ID |
| `identity_id` / `credential_record_id` | TEXT FK | 身份与登录凭据 |
| `app_id` / `origin` | TEXT | 宿主和来源作用域 |
| `token_hash` | BLOB UNIQUE | 随机会话令牌摘要 |
| `remember_30d` | INTEGER | 本次登录期限选择 |
| `created_at` / `authenticated_at` | INTEGER | 时间信息 |
| `reauthenticated_at` | INTEGER NULL | 近期验证 |
| `last_seen_at` / `expires_at` | INTEGER | 活跃与绝对截止时间 |
| `user_agent` | TEXT NULL | 可选、长度受限的设备提示 |
| `ip_metadata` | TEXT NULL | 默认不保存，启用后仅用于审计 |
| `revoked_at` | INTEGER NULL | 撤销标记 |

#### `setup_authorizations`

主要字段：`id`、`identity_id`、`app_id`、`origin`、`token_hash`、`status`、`browser_binding_hash`、`created_at`、`expires_at`、`version`。

`status` 仅表示尚可用的 issued / bound 状态。消费、撤销或过期删除摘要和授权数据；审计表单独记录事件类型和授权 ID，不保存已使用 Token 的明文或摘要历史。

#### 其他认证数据

`security_events` 保存事件 ID、事件类型、时间、宿主、相关非秘密记录 ID、通用结果码和 request ID。`auth_schema_migrations` 独立记录认证库版本。

Challenge 默认不存入这些主表。临时状态容量与 TTL 由可替换的 `ChallengeStore` 管理。[A1 §19]

### 15.5 双数据库边界

业务库不通过跨数据库 FK 依赖认证库。删除或恢复业务数据不改变 RP 身份，清空 Session 不改变收藏。

分别生成的一致性快照不天然构成一个跨数据库同一时刻的事务快照。备份清单必须记录各自时间；需要严格协调时，应暂停相关写入后执行，而不是宣称两个独立备份文件已经原子一致。

### 15.6 数据库迁移

应用启动前检查 schema 支持范围和迁移校验和。迁移前生成可恢复备份；失败不启动业务读写，不反复尝试破坏性迁移。

共享 `auth.db` 的迁移由显式 `auth migrate` 负责，持有跨进程迁移锁，并要求其他使用者停止或进入兼容模式。每个宿主都拒绝读取 / 写入自己不理解的较新认证 schema 或凭据序列化版本。

不承诺“替换旧二进制就能任意降级”；涉及不兼容迁移时，回滚使用匹配的程序和迁移前备份。

---

<a id="section-16"></a>

## 16. HTTP API 设计

### 16.1 通用规范

业务 API 前缀为 `/api/v1`；默认认证路由沿用附件，由宿主绑定，核心模块不写死路径。[A1 §18]

数据采用 JSON，时间和 ID 规则见第 15 章。默认同源，不开启跨域读取。所有私人响应 `Cache-Control: no-store`；所有写入进行 Session / 注册授权、Origin、CSRF 或预认证绑定检查。

GET 不创建收藏、不撤销会话、不消费 Token。HTTP API 返回正确状态码，不统一包装成 200。

统一错误格式：

```json
{
  "error": {
    "code": "VERSION_CONFLICT",
    "message": "该收藏已在其他设备更新，请重新加载后处理。",
    "request_id": "opaque-request-id",
    "details": {}
  }
}
```

### 16.2 认证端点

| 方法与路径 | 访问前提 | 行为 |
|---|---|---|
| `GET /login` | 无 | 极简无数据页面 |
| `GET /api/v1/session` | 可匿名调用 | 返回 authenticated；已登录时返回有限 Session 摘要和 CSRF Token |
| `POST /auth/login/begin` | 同源、限流、预认证绑定 | 接收 remember_30d，返回登录 options 与操作 ID |
| `POST /auth/login/finish` | 配对 Challenge 与绑定 | 验证后设置 Cookie；响应体不含 Session Token |
| `GET /setup` | 模式和有效授权允许 | 返回 Token 输入页面，否则不可用 |
| `POST /auth/setup/verify` | 同源、限流 | 验证并绑定一次性注册授权 |
| `POST /auth/register/begin` | 有效绑定授权 | 返回注册 options |
| `POST /auth/register/finish` | 授权、Challenge、模式仍有效 | 保存 credential 并原子消费授权 |
| `POST /auth/reauth/begin` | 有效 Session、CSRF | 对当前 Session 发起再认证 |
| `POST /auth/reauth/finish` | 有效 Session、配对 Challenge | 更新近期验证状态，不延长会话 |
| `POST /logout` | Session、CSRF | 撤销当前 Session 并清 Cookie |
| `GET /api/v1/auth/credentials` | Session | 只读名称和必要摘要，无公钥原始状态 |
| `PATCH /api/v1/auth/credentials/{id}` | Session、CSRF、近期验证 | 修改名称，不更改密钥材料 |
| `GET /api/v1/auth/sessions` | Session | 当前宿主 Session 列表 |
| `DELETE /api/v1/auth/sessions/{id}` | Session、CSRF、近期验证 | 撤销当前宿主指定 Session |
| `POST /api/v1/auth/sessions/revoke-others` | Session、CSRF、近期验证 | 撤销本宿主其他 Session |

匿名 Session 检查只返回 `authenticated: false`，不泄露收藏数量、凭据列表、内部数据库状态或恢复详情。AuthStore 异常返回 503，不冒充正常未登录。

登录 begin 示例：

```json
{ "remember_30d": true }
```

认证响应包装示意：

```json
{
  "ceremony_id": "opaque-random-id",
  "expires_at": "2026-09-19T10:05:00Z",
  "publicKey": {
    "challenge": "base64url-value",
    "rpId": "example.com",
    "userVerification": "required"
  }
}
```

实际 options / credential 响应由适配器按标准完整生成，上述示意不要求开发者手写精简版 WebAuthn 协议。

### 16.3 收藏与分类端点

| 方法与路径 | 用途 |
|---|---|
| `GET /api/v1/bookmarks` | 列表、搜索、目录 / 标签 / 置顶筛选、分页 |
| `GET /api/v1/bookmarks/{id}` | 单条详情 |
| `POST /api/v1/bookmarks` | 新建，支持幂等键 |
| `PATCH /api/v1/bookmarks/{id}` | 编辑，必须携带版本 |
| `DELETE /api/v1/bookmarks/{id}` | 移入回收站，必须携带版本 |
| `POST /api/v1/bookmarks/{id}/restore` | 恢复，处理目标目录冲突 |
| `POST /api/v1/bookmarks/batch` | 批量移动、标签或删除，含每条预期版本 |
| `POST /api/v1/bookmarks/reorder` | 重排受影响目录 / 置顶区 |
| `GET /api/v1/folders` | 有效目录树 |
| `POST /api/v1/folders` | 新建目录 |
| `PATCH /api/v1/folders/{id}` | 重命名或移动，检查环与版本 |
| `DELETE /api/v1/folders/{id}` | 按显式策略处理子内容 |
| `POST /api/v1/folders/{id}/restore` | 恢复目录或删除批次 |
| `POST /api/v1/folders/reorder` | 目录同级排序 |
| `GET /api/v1/tags` | 标签列表 |
| `POST /api/v1/tags` | 创建标签 |
| `PATCH /api/v1/tags/{id}` | 重命名与合并冲突提示 |
| `DELETE /api/v1/tags/{id}` | 删除关联标签，不删除收藏 |
| `GET /api/v1/library/revision` | 轻量更新检查 |
| `GET /api/v1/trash` | 回收站列表 |
| `POST /api/v1/trash/purge` | 近期验证后永久删除所选或全部 |

路径中的静态段 `batch`、`reorder` 必须优先于动态 ID 匹配，或使用严格 ID 校验，避免路由歧义。

### 16.4 导入导出与设置端点

| 方法与路径 | 用途 |
|---|---|
| `POST /api/v1/imports` | 上传 HTML / JSON，生成预览作业 |
| `GET /api/v1/imports/{id}` | 预览、警告、当前状态和结果报告 |
| `POST /api/v1/imports/{id}/commit` | 确认目标与策略后幂等提交 |
| `DELETE /api/v1/imports/{id}` | 取消未提交作业并清理暂存 |
| `POST /api/v1/exports` | 近期验证后生成 HTML / JSON 下载响应 |
| `GET /api/v1/preferences` | 读取业务偏好 |
| `PATCH /api/v1/preferences` | 更新偏好及版本 |
| `GET /healthz` | 最小存活检查，无内部详情 |

备份认证库、恢复数据库和迁移 schema 仅由 CLI 提供，不开放为普通网页上传任意数据库的管理接口。

### 16.5 状态码

400 参数错误；401 未认证 / Session 过期；403 Origin、CSRF 或近期验证不满足；404 资源不存在或 Setup 不可用；409 版本、重复提交内容或状态冲突；413 体积超限；422 文件 / 数据格式不可处理；429 限流；503 存储或服务暂不可用。

WebAuthn 内部原因映射为有限的公开错误码，详细安全判断只进入脱敏审计，不向匿名请求说明某个 credential 是否存在。[A1 §23]

---

<a id="section-17"></a>

## 17. 配置、CLI 与管理流程

### 17.1 配置原则

支持 `--config <file>`，优先级为显式 CLI 参数、带项目命名空间的环境变量、配置文件、默认值。路径相对配置文件目录解析；没有配置文件时相对显式 `data_dir`，不依赖服务管理器的偶然工作目录。

敏感值尽量通过文件提供。User ID 是稳定标识，不是密码，但仍不随意上传公开配置库。数据目录和认证库路径必须显式可见，错误路径不能偷偷触发初始化。

### 17.2 生产配置示例

```toml
[server]
listen = "127.0.0.1:8765"
public_url = "https://bookmark.example.com"
trusted_proxies = ["127.0.0.1/32", "::1/128"]

[storage]
data_dir = "."
business_db = "bookmarks.db"

[auth]
app_id = "bookmarkd"
rp_id = "example.com"
rp_name = "Private Services"
user_id_file = "identity/user-id.txt"
user_name = "owner"
display_name = "owner"
allowed_origins = ["https://bookmark.example.com"]
setup_mode = "disabled"
setup_token_ttl_seconds = 600
challenge_ttl_seconds = 300
session_short_ttl_seconds = 43200
session_long_ttl_seconds = 2592000
recent_auth_ttl_seconds = 300

[auth.store]
kind = "sqlite"
path = "auth.db"

[security]
csrf_key_file = "secrets/csrf.key"
allow_insecure_localhost = false

[features]
metadata_fetch = false

[logging]
level = "info"
format = "json"
log_query_strings = false
log_client_ip = false
```

`user-id.txt` 和 `csrf.key` 由初始化命令生成或加载，不使用文档中的示意字符串代替真实随机数据。生产配置中长期会话 TTL 必须与页面“30 天”一致；不允许后台改为 90 天而前端仍标 30 天。

### 17.3 命令清单

以下均为待实现 CLI 契约：

| 命令 | 行为 |
|---|---|
| `bookmarkd init` | 显式初始化新数据目录、业务库、认证身份和本地密钥；拒绝无提示覆盖 |
| `bookmarkd serve` | 前台运行，由 OS 服务管理器托管 |
| `bookmarkd config validate` | 校验配置、路径、RP / Origin 和身份一致性，不输出秘密 |
| `bookmarkd doctor` | 检查数据库可访问性、版本、权限与配置；不主动注册或迁移 |
| `bookmarkd auth list` | 列出 credential ID / 管理 ID、名称和状态 |
| `bookmarkd auth rename <id> <name>` | 重命名凭据 |
| `bookmarkd auth revoke <id>` | 撤销凭据及其所有宿主相关 Session |
| `bookmarkd auth revoke-all` | 撤销共享身份所有凭据和 Session，需明确确认 |
| `bookmarkd auth create-setup-token` | 输出一次性 Token 和无 Token 的 Setup URL |
| `bookmarkd auth sessions` | 列出当前宿主 Session |
| `bookmarkd auth revoke-session <id>` | 撤销本宿主指定 Session |
| `bookmarkd auth revoke-all-sessions` | 撤销本宿主全部 Session |
| `bookmarkd auth revoke-all-sessions --all-apps` | 显式撤销共享身份所有宿主 Session |
| `bookmarkd auth migrate` | 管理认证库迁移，要求其他使用者停止或满足兼容条件 |
| `bookmarkd backup --output <path>` | 默认仅备份业务数据和清单 |
| `bookmarkd backup --include-auth --output <path>` | 显式包含认证身份和凭据，清除备份副本中的 Session / Setup 授权 |
| `bookmarkd restore --input <path>` | 离线校验并恢复，默认不覆盖认证身份 |
| `bookmarkd version` | 输出版本、提交号、目标架构和 schema 支持范围 |

破坏性命令非交互执行时要求 `--yes` 等明确标志。全凭据撤销的影响范围始终显示为共享身份，不因为命令由收藏服务执行就假装只影响收藏服务。

### 17.4 首次初始化示例

以下示例需把域名和目录换成实际值；路径中不包含真实凭据。

```bash
bookmarkd init \
  --data-dir ./data \
  --public-url https://bookmark.example.com \
  --rp-id example.com \
  --rp-name "Private Services" \
  --user-name owner \
  --setup-mode auto

bookmarkd --config ./data/config.toml config validate
bookmarkd --config ./data/config.toml serve
```

随后在另一个管理员终端执行：

```bash
bookmarkd --config ./data/config.toml auth create-setup-token
```

浏览器访问输出的 `/setup` 地址，输入单独展示的 Token 完成注册。注册后验证登录，再把持久配置中的 `setup_mode` 改为 `disabled` 并重启。

### 17.5 后续宿主接入

后续宿主配置相同 RP / User 身份，使用同一权威 AuthStore，设置自己的 `app_id` 和 Origin，保持 `setup_mode=disabled`。启动时只做身份核对，不再调用“首次初始化并生成 User ID”。

本机共享时可以把 `auth.store.path` 指向受权限保护的公共认证目录，User ID 从同一身份资料加载；不通过 NAS 共享 SQLite。

---

<a id="section-18"></a>

## 18. 安全、隐私与防滥用

### 18.1 全资源访问保护

除明确列出的公共页面壳、认证入口和最小健康检查外，所有私人数据请求均经过认证，包括收藏列表、搜索、目录、标签、图标、导入报告和导出。

静态页面不得内嵌私人数据或服务端密钥。`robots.txt`、隐藏路径和复杂 URL 不构成访问控制。

### 18.2 HTTPS 与反向代理

生产浏览器入口必须 HTTPS。反向代理终止 TLS 后通过本机受信任连接转发可以采用 HTTP，但应用使用配置中的外部 Origin 验证 WebAuthn，不能把内部监听地址当成 RP Origin。[A1 §14；本项目部署细化]

仅在连接来自显式信任的代理时采信转发头；代理应覆盖外部传入的同名头。未知 Host 拒绝处理，不根据任意 Host 生成 Setup URL 或 Origin 白名单。

### 18.3 CSRF

SameSite 是额外防护，不替代所有写接口的 CSRF 检查；同站的其他子域也不是自动可信来源。[R10]

普通 Session 写操作需要精确 Origin 检查和 `X-CSRF-Token`。本项目可使用下述派生方式，避免将认证令牌暴露给脚本：

```text
csrf = HMAC-SHA256(
  每个宿主独立的持久 CSRF 密钥,
  带长度编码的 app_id || origin || session_id || session_token_hash
)
```

`GET /api/v1/session` 仅向本源已登录脚本提供 CSRF 值，不开放 CORS。CSRF 密钥保存在本地受保护文件，首次显式初始化生成；它不是 Passkey 私钥，也不用于 WebAuthn 验签。

登录和 Setup 的匿名 POST 采用 JSON、精确 Origin、随机预认证 Cookie 绑定和限流。注册还需一次性授权；不能把未登录作为不检查请求来源的理由。

### 18.4 XSS 与浏览器防护

标题、网址、目录、标签、备注、导入文件和未来抓取结果都视作不可信输入，按文本渲染；禁止将备注直接插入 `innerHTML`。链接跳转前仍执行 scheme 校验，导入器的判断不能替代展示层防护。

采用自托管脚本与样式、限制内容安全策略、`frame-ancestors 'none'`、`object-src 'none'`、`base-uri 'none'`、`X-Content-Type-Options: nosniff` 和 `Referrer-Policy: no-referrer`。具体 CSP 根据构建产物验证，不为省事开启任意第三方脚本或 `unsafe-eval`。

### 18.5 可选元信息抓取

V1 默认不抓取。后续实现时，它是受保护、可关闭的增强模块，不影响收藏保存结果。

服务端访问用户提供 URL 存在 SSRF 风险，需要同时检查输入、解析地址、实际连接目标和重定向链。[R11]

本项目抓取策略：仅 HTTP(S)，仅明确允许的公网目标；拒绝回环、私网、链路本地和其他特殊地址，包括相应 IPv6 和映射形式。每次重定向重新核验，最多 3 次；DNS 校验后连接必须固定到已经批准的目标地址，保留正确的 Host / TLS SNI，防止解析与实际连接目标分离。

默认不继承环境代理、不携带用户 Cookie / Authorization，不读取目标网站的登录页面，不执行网页 JavaScript。初始总超时 10 秒、正文 1 MiB、有限并发；只处理允许的内容类型。失败只返回“未能获取信息”，不覆盖手工字段。

图标从本地缓存提供；不向第三方图标服务批量暴露收藏域名。外部 SVG 不作为同源可执行资源直接提供；图片解码、像素和文件大小均设上限，优先安全转码后的位图。

### 18.6 限流与输入上限

| 资源 | 初始策略 |
|---|---|
| 登录 begin / finish | 每 IP 每分钟 20 次，结合浏览器绑定和全局上限 |
| Setup 验证 | 每 IP 每分钟 5 次，外加全局上限，返回通用错误 |
| 未完成 Challenge | 每浏览器最多 5 个，全局最多 1,000 个，TTL 自动清理 |
| 认证 JSON | 最大 256 KiB |
| 普通业务 JSON | 最大 1 MiB |
| 导入上传 | 最大 20 MiB |
| 导入提交 | 单应用同时最多 1 个 |

限流源地址只来自直连地址或受信代理，不能被任意转发头绕过。不要对单用户设置可被匿名攻击者无限延长的永久锁号；限流是暂时拒绝，不删除凭据。

### 18.7 日志与审计

默认记录时间、级别、事件类型、路由模板、HTTP 状态、耗时、request ID 和非秘密实体 ID。默认不记录 URL 查询字符串、完整目标网址、搜索词、请求 / 响应体、Cookie、认证令牌或 Challenge。

审计包括初始化、注册成功、登录结果类别、凭据撤销、会话撤销、敏感导出、备份 / 恢复和迁移。日志保留时间与大小有上限，初始建议 30 天；IP 默认关闭采集，启用时明确说明。

### 18.8 客户端数据残留

V1 不使用 Service Worker 缓存私人 API，不把整个收藏库存入 localStorage / IndexedDB。静态资源可以按内容哈希缓存，私人数据响应禁止缓存。

主动退出后撤销服务端 Session、清 Cookie、清前端业务状态与待添加草稿。多标签页通过本源消息通知清理；页面从后退缓存恢复时先重新验证 Session，在验证前隐藏旧私人内容。

V1 不把 `Clear-Site-Data: "cookies"` 作为默认注销手段，以免产生跨同站其他服务的非预期 Cookie 清理。使用精确 Cookie 删除和应用自身状态清理。

### 18.9 服务器与备份边界

程序无需以管理员 / root 身份运行。数据目录和认证文件限制读写；多个可信本机服务共享时采用明确的服务账号 / 组或 ACL，而不是开放所有用户读写。

登录验证不等于磁盘加密。备份包含私人收藏；需要更强静态保护时使用操作系统磁盘加密或可信加密备份工具，不自创加密格式。

---

<a id="section-19"></a>

## 19. 部署、备份、恢复与升级

### 19.1 单文件程序和运行数据

“单文件”指一个可执行制品，不代表运行后永远只产生一个文件。前端 JS、CSS、本地图标、默认模板和迁移脚本嵌入二进制；用户不需要另行安装前端目录。[R5]

运行数据示例：

```text
bookmarkd                 # Windows 为 bookmarkd.exe

data/
├── config.toml
├── bookmarks.db
├── auth.db               # 共享配置时可位于其他本地受保护目录
├── identity/
│   └── user-id.txt
├── secrets/
│   └── csrf.key
├── cache/                # 可删除的未来图标等缓存
├── tmp/                  # 受控导入暂存
└── backups/              # 可配置到其他路径
```

SQLite 运行时产生 WAL / SHM 等辅助文件是正常现象。数据库目录不宜只允许主 `.db` 文件写入而禁止建立辅助文件。

### 19.2 进程管理

程序默认前台运行并响应正常终止信号；Linux、macOS、Windows 分别交给系统服务管理方式托管。V1 文档提供服务配置示例，不要求程序首发就提供跨平台提权安装器或自更新守护程序。

启动顺序：解析配置、校验权限与路径、核对身份 / schema、获取本业务实例锁、初始化服务、监听端口。异常退出时给出明确错误；停止时停止接受新任务，结束或安全中断正在运行的作业。

### 19.3 SQLite 放置

运行中业务库和默认认证库放在本地磁盘。NAS 保存备份，不作为多机器并发运行 SQLite 的目录。WAL 依赖同机协作机制，不适合网络文件系统共享。[A1 §21；R3]

### 19.4 备份

使用 SQLite Online Backup API 或同等一致性机制获得快照；不要直接复制活跃 WAL 模式下的主数据库文件后宣称得到完整备份。[R7]

备份清单包含程序版本、业务 / 认证 schema 版本、身份摘要、各快照时间、文件摘要和是否包含认证资料。写入临时文件后校验，最后原子重命名到目标，失败不覆盖上一次有效备份。

默认备份仅包括业务数据。显式 `--include-auth` 备份认证身份与 credential；在**备份副本**中删除 Session、Setup 授权和短时认证状态，不影响正在运行的服务。

业务和认证分别一致但不保证同一瞬间，见第 15.5 节。需要整体停机点时协调所有写入者；共享认证库不能只暂停收藏服务便假定已经没有其他写入。

### 19.5 恢复

恢复必须离线或进入维护状态。校验清单、摘要、schema、目标路径和已有身份，先保留当前数据备份。压缩备份的解包禁止绝对路径和 `..` 路径穿越。

默认只恢复业务数据；恢复认证资料需显式确认影响整个共享身份。恢复后清除全部 Session / Setup 状态，并生成本宿主新的 CSRF 密钥。

恢复旧认证备份可能恢复到旧的 credential 有效集合，管理员必须复核备份之后撤销过的凭据，不能误把已丢失的旧密钥重新投入使用。工具应展示这一风险和凭据清单，要求明确确认。

服务器备份不包含客户端 Passkey 私钥，不能代替认证器或 Passkey Provider 的可用性；全部私钥不可用时仍走 SSH 恢复。[A1 §7、§16]

### 19.6 升级

先备份，再替换程序并完成受控迁移。共享认证库的升级协调与业务库分别处理。数据库迁移失败时保留可恢复状态并停止提供业务写入。

不实现静默自动升级。用户应能核对制品版本、校验和、依赖说明及恢复方法。

### 19.7 网络可达性

浏览器需要能连接收藏服务。域名、HTTPS、DDNS、IPv4 / IPv6 连通性由部署层保证；项目不内置穿透、VPN 或 DNS 服务。

部署系统支持的操作系统和使用浏览器的操作系统可以不同。只需在一台常驻机器运行本应用，其他设备访问同一网址。

---

<a id="section-20"></a>

## 20. 技术选型与跨平台制品

### 20.1 后端与数据库

采用 Rust 的主要理由是延续已有项目路线、明确模块边界和跨平台二进制交付，不是本项目需要极端吞吐。Axum / Tokio 放在宿主与适配器层；领域和认证核心保持可测试、可复用。

SQLite 默认驱动选择 `rusqlite`，使用 bundled SQLite 与备份功能的构建选项；由有界数据库执行层隔离同步调用。[R6]

### 20.2 前端

TypeScript + Svelte + Vite，构建为静态站点资源，由 Rust 提供。Svelte 使用编译型组件路线，Vite 提供静态生产构建；本项目不使用需要运行 Node.js 的服务端渲染部署。[R12、R13]

构建机可安装 Node.js、Rust 和 C 工具链，运行目标机器不需要这些开发工具。禁止运行时从 CDN 加载核心 JS、CSS、字体或依赖库。

### 20.3 WebAuthn 库：先验证适配，不降低要求

Rust 首选候选是 `webauthn-rs` 的高层安全封装，由独立 `WebAuthnEngine` 适配器接入。其文档提供注册、验证、服务端配对状态和 Origin 配置接口；具体 API 和特性必须对锁定版本核对。[R4、R14]

**该候选不等于已经证明与本文所有参数完全一致。**当前核对的高层注册签名使用 UUID 型 User ID，普通 Passkey API 的可发现凭据策略也不能只从名称推断。因此在 M0 中必须验证：完整 User Handle、`residentKey=required`、UV required、无用户名的已知凭据列表登录、可发现登录支持边界、同步凭据、精确 Origin 白名单和手机跨设备流程。

若选定版本高层接口无法原样实现需求，应记录依赖选型决定并替换为满足要求的成熟验证库 / 经审查适配方案；不截断已有 User ID、不降级 UV、不改成密码登录，也不为了保留库名静默改变附件要求。直接调用低层核心需要额外安全审查，不能被当作无风险捷径；该项目低层库自身提示优先使用安全封装。[R15]

这是设计阶段的依赖验证关口，不是要求自行实现 attestation、CBOR、COSE 或密码学。具体版本在兼容性验证后锁入 `Cargo.lock`，并形成可复现测试报告。

### 20.4 单文件的原生依赖

前端内嵌不自动等于完全没有原生动态依赖。SQLite、WebAuthn 验证库可能使用的密码学后端、系统运行库均要核查。

如果依赖树包含 OpenSSL，可以评估其 vendored 静态构建方案，但仍必须在每个平台实际检查链接结果；不能声称“HTTP 使用 rustls，所以整个程序必然没有 OpenSSL”。[R16]

Linux 目标优先评估 musl 静态构建；Windows 核查 CRT 和 DLL 依赖；macOS 允许使用操作系统自带框架，但不依赖用户另外安装 Homebrew 库。目标是无额外应用运行时安装，不是脱离操作系统运行。

### 20.5 发布矩阵

| 平台 | V1 目标架构 | 制品命名示意 |
|---|---|---|
| Linux | x86_64 | `bookmarkd-linux-x86_64` |
| Linux | aarch64 | `bookmarkd-linux-aarch64` |
| macOS | arm64 | `bookmarkd-macos-arm64` |
| macOS | x86_64 | `bookmarkd-macos-x86_64` |
| Windows | x86_64 | `bookmarkd-windows-x86_64.exe` |

每个制品清单注明实际验证的最低系统版本和依赖。Windows ARM64 为后续扩展，不在 V1 强行承诺。

使用各平台构建环境或经验证的交叉工具链，不假定一台 Linux 主机上的同一命令天然生成全部 macOS / Windows 可分发制品。

### 20.6 构建流水线

```text
锁定依赖与工具链
→ 前端类型检查、单元测试
→ npm ci 与前端 production build
→ Rust fmt / clippy / 单元测试 / 集成测试
→ 嵌入前端资源与迁移脚本
→ 各目标平台 release 构建
→ 依赖 / 链接检查
→ 干净环境启动、数据库、认证与静态页面测试
→ 生成校验和、版本清单、许可证与测试报告
```

`npm ci`、Cargo locked 构建等命令须在实现仓库提供可直接执行的脚本。离线构建不是 V1 默认承诺；如增加，需额外交付 vendored 依赖、工具链和缓存，不能认为已有一个源代码目录就可离线编译。

签名 / notarization 可作为发布流水线能力，但没有实际证书和签名结果前不声称所有系统会无提示信任制品。

---

<a id="section-21"></a>

## 21. 非功能需求

### 21.1 性能目标

基准：1 vCPU、1 GiB 内存、本地 SSD、10,000 条收藏、200 个目录、500 个标签、平均备注 256 字符，5 个活跃浏览器会话。所有指标必须注明硬件、操作系统、构建版本和测试方式。

| 指标 | 初始验收目标 |
|---|---|
| 列表 / 搜索 API | 热数据、本机网络条件下 p95 ≤ 200 ms |
| 普通单条写入 | 非批量导入期间 p95 ≤ 200 ms |
| 稳态内存 | 目标 ≤ 256 MiB RSS，按实际平台记录 |
| 前端核心 JS | gzip 后目标 ≤ 300 KiB，超出需说明 |
| 更新感知 | 页面重新获得焦点后执行一次版本检查并按需刷新 |
| 重启 | 已提交业务数据和未过期 Session 持久保留；易失 Challenge 安全失效 |

Passkey 的人工确认、手机扫码和远端网络耗时不计入业务 API 处理延迟。以上是设计目标，不是已测结果。

### 21.2 可用性与可靠性

任何“保存成功”必须表示事务已经提交。写失败不能乐观显示永久成功。导入 / 恢复异常有明确报告；磁盘满和数据库锁超时不造成进程崩溃或匿名放行。

暂无高可用、多实例业务副本或零停机迁移要求。默认可维护性优先于复杂分布式能力。

### 21.3 可访问性

登录按钮和勾选项可键盘操作、有可见焦点和关联标签。异步错误采用可读文本，不仅依赖颜色。移动端不依赖 hover；列表和弹窗能合理处理系统字体放大。

### 21.4 本地化

首版界面以简体中文为主，字段和代码标识使用稳定英文。日期按浏览器本地时区显示，服务端存 UTC；认证到期由服务端时钟决定，不信任客户端传入的时间。

---

<a id="section-22"></a>

## 22. 测试与验收矩阵

### 22.1 功能与数据验收

| ID | 场景 | 必须满足 |
|---|---|---|
| B01 | 导入真实浏览器 HTML | 收藏数量、中文、目录、网址可核对，失败项有报告 |
| B02 | 同 URL 不同目录 / 备注 | 不被默认强制合并或删除 |
| B03 | 重复提交 / 网络重试 | 幂等，不新增意外副本 |
| B04 | 一两个汉字、英文大小写、多个词 | 按第 6 章语义命中 |
| B05 | 手机新增，电脑重新聚焦 | 能获取新增内容，未保存表单不被覆盖 |
| B06 | 两端同时编辑 | 后提交者收到明确版本冲突，不静默覆盖 |
| B07 | 文件夹移入自己的后代 | 被拒绝，不产生目录环 |
| B08 | 批量操作中途失败 | 按事务策略回滚，报告结果一致 |
| B09 | 删除、恢复目录及收藏 | 不残留不可见有效条目；冲突有提示 |
| B10 | JSON 导出再导入 | 支持字段往返完整，ID 关联正确 |
| B11 | HTML 导出 | 能被目标浏览器实际导入 |
| B12 | 程序升级或异常重启 | 已提交数据不丢失，失败不报告成功 |
| B13 | 导入恶意 HTML / 禁止 scheme | 不执行、不抓取，出现在报告中 |
| B14 | 内网 URL | 能保存；服务器不会默认探测内网 |
| B15 | 登出与返回历史页 | 旧私人内容不再直接展示，需重新检查 Session |

### 22.2 登录页与会话验收

| ID | 场景 | 必须满足 |
|---|---|---|
| A01 | 未登录常态页面 | 仅一个 Passkey 按钮和一个 30 天勾选项，没有密码 / 注册 / 恢复入口 |
| A02 | 不勾选登录 | 会话 Cookie；服务端 12 小时绝对到期 |
| A03 | 勾选登录 | Max-Age 2,592,000 秒；服务端同样 30 天截止 |
| A04 | 持续访问第 29 天 | 不滑动续期至第 59 天 |
| A05 | Cookie 被清理 | 重新 Passkey 登录，不凭本地偏好自动认证 |
| A06 | 当前会话注销 | 服务端撤销且精确清 Cookie |
| A07 | 撤销其他会话 | 目标下一个请求失败，本会话不受意外影响 |
| A08 | 撤销登录凭据 | 该凭据派生的各宿主 Session 同时失效 |
| A09 | 跨 IP / 网络 | 不仅因为 IP 变化而错误注销 |
| A10 | 再认证 | 必须绑定当前 Session，不延长原绝对到期时间 |
| A11 | 用户取消 Passkey | 可重试，不新增控件、不出现密码退路 |
| A12 | 不支持 WebAuthn | 明确提示，不暴露私人数据 |
| A13 | 外部 Bookmarklet + 有效 Strict Cookie | 本源会话检查后无需错误要求重复登录，草稿完整 |
| A14 | 未登录带入 URL | 认证后恢复本次待添加字段，不自动保存 |

### 22.3 Setup、重放与共享认证验收

| ID | 场景 | 必须满足 |
|---|---|---|
| S01 | disabled + 有效 Token | 仍不能注册 |
| S02 | enabled + 无 Token | 不能注册 |
| S03 | auto + 空凭据 + 有效 Token | 能注册第一个凭据，之后 Setup 关闭 |
| S04 | Token 错误 / 过期 / 已用 | 全部拒绝，公开错误不泄露内部细节 |
| S05 | 两请求并发使用同一注册授权 | 最多创建一个 credential |
| S06 | 两宿主 auto 并发初始化 | 事务保证首次初始化规则，不分别注册两个“首个用户” |
| S07 | 令牌绑定后换浏览器提交 | 拒绝 |
| S08 | 相同 assertion 重放 | 最多首次成功 |
| S09 | 注册响应用于登录 / Challenge 过期 | 拒绝 |
| S10 | Origin 不在白名单 / 端口变化 | 拒绝 |
| S11 | RP ID / User ID 与已存身份不一致 | 启动失败，不重生成身份 |
| S12 | 未完成 UV | 拒绝 |
| S13 | counter 始终为 0 | 不仅因此拒绝合法凭据 |
| S14 | counter 回退与并发更新 | 记录安全事件，按库语义处理，无旧状态覆盖 |
| S15 | 登录验证期间凭据被撤销 | 不得创建有效 Session |
| S16 | 第二宿主相同身份及 AuthStore | 无需 Setup，原 Passkey 可登录 |
| S17 | A 登录后直接访问 B | B 无自己的 Session 时仍要求登录 |
| S18 | 手工将 A 的 Session Token 交给 B | 作用域检查拒绝 |
| S19 | 只匹配四个身份值但凭据库为空 | 不能假装能够使用原凭据直接登录 |
| S20 | AuthStore 不可用 | 503，永不匿名放行 |
| S21 | 进程重启 | Challenge 失效；正常 Session 持久可用 |
| S22 | 日志审查 | 无 Session / Setup Token、Cookie、Challenge 或请求体秘密 |
| S23 | CLI Token 与运行进程 | 持久化摘要能协作，不依赖两个进程“共享内存”的错误假设 |
| S24 | 原始 User Handle | 完整字节往返一致，不能因库适配被截断 |

### 22.4 浏览器与认证器实测

至少记录以下环境的注册、登录、30 天 Cookie 行为、取消和失败重试：Safari / macOS 与 iOS、Chrome / Chromium、Edge / Windows、Firefox、Android 浏览器、支持 UV 与可发现凭据的 FIDO2 USB 安全密钥、浏览器提供的手机跨设备认证。[A1 §25]

不要求每种浏览器都支持完全相同的本地 Passkey Provider，但必须明确实测可用路径。不能用“某个浏览器内核的自动化用例通过”替代真实手机、Windows Hello、系统钥匙串或 USB 设备测试。

自动化可使用虚拟认证器覆盖协议和状态；实机测试记录浏览器版本、系统版本、认证器类型及已知限制，不记录私钥或敏感用户数据。

### 22.5 安全、发布与恢复验收

未认证无法查询任何私人数据；写接口 CSRF、精确 Origin 和限流有效；恶意标题 / 导入内容不执行；上传超限返回明确错误；可选抓取未开启时无出站请求。

每个制品在没有 Node.js、Python、Rust 编译器和额外数据库服务的干净环境启动。检查动态依赖，前端不依赖外部 CDN。

至少执行一次真实备份恢复演练，包括业务恢复、显式认证恢复、Session 失效、旧认证状态风险提示和身份一致性检查。共享认证库迁移必须测试两个宿主版本不兼容时的拒绝行为。

---

<a id="section-23"></a>

## 23. 实现顺序与里程碑

### M0：认证和制品可行性验证

先完成一个最小宿主的注册 / 登录闭环，验证 32 字节 User Handle、discoverable 策略、UV、手机认证、Origin 和关键目标平台构建。锁定 WebAuthn 依赖及状态序列化格式，记录第 20.3 节中的适配决定。

产出不是业务界面演示，而是能够阻止错误技术假设进入后续开发的可运行认证测试。

### M1：工程骨架与可复用认证模块

建立 workspace、嵌入式静态页面、配置与身份初始化、默认 AuthStore、ChallengeStore、极简登录、Setup、CLI 恢复、Session 和第二宿主夹具。

退出条件：A01～A12 及核心 S 系列测试通过；默认未认证无法得到任何业务数据；没有密码后门。

### M2：收藏基本闭环

实现收藏、文件夹、标签、备注、置顶、搜索、手机列表和基本回收站。

退出条件：可以在两个浏览器进行完整增查改删，搜索短词正确，编辑冲突可见。

### M3：迁移与低摩擦添加

实现 HTML / JSON 导入导出、预览报告、幂等、批量整理、Bookmarklet、登录续接和 Strict Cookie 外部导航测试。

退出条件：真实收藏可迁入、往返、恢复，不需要重新整理所有目录；未登录带入链接不会丢失。

### M4：安全、运维与跨平台发布

补齐 CSRF、日志脱敏、限流、备份恢复、迁移、干净环境测试、浏览器实测和制品检查。

退出条件：第 22 章 V1 必须项通过，交付源码、制品、校验和、运行说明与实测限制。

### M5：可选增强

按使用反馈增加别名、批量粘贴、分享入口和元信息抓取。新增能力不得改变极简登录页和单用户边界。

---

<a id="section-24"></a>

## 24. 完成交付定义与开发约束

### 24.1 V1 完成定义

不是“页面可以打开”便完成，而是能够稳定走通：

```text
管理终端初始化
→ 浏览器注册 Passkey
→ 极简登录
→ 导入已有收藏
→ 多设备查找 / 打开 / 添加 / 整理
→ 安全注销与撤销
→ 备份 / 升级 / 恢复
→ 另一个宿主复用同一认证模块和 credential
```

交付物包括应用源码、可复用认证模块、默认存储适配器、前端源码、各平台制品、迁移、测试、配置样例、CLI 帮助、认证集成文档、备份恢复说明和实际兼容性报告。

### 24.2 不允许的捷径

不得新增密码或 TOTP 作为方便测试的生产登录入口；不得让“仅自己用”成为取消认证、输入校验、CSRF 或备份的理由；不得把认证模块独立启动成必须依赖的服务。

不得用匿名可访问的业务 API 配合前端隐藏页面冒充保护；不得把 Session / Setup Token 放进 Bookmarklet；不得把所有子域 Cookie 打通冒充共享 Passkey；不得自行实现底层 FIDO、签名算法或自定义扫码登录协议。

不得为了单文件宣传而隐藏运行所需的额外 DLL / 动态库；不得在没有实测时声称支持任意设备、任意浏览器或任意系统版本。

### 24.3 需求变更原则

先保证稳定的登录、快速打开、低摩擦保存、可靠迁移。任何新增功能若要求引入独立服务、用户系统、离线同步、无头浏览器或 AI 运行时，都需要单独评估，不可作为普通“小优化”自动纳入。

---

<a id="section-25"></a>

## 25. 认证附件覆盖与补充决策对照

### 25.1 [A1] 原章节覆盖

| 附件章节 | 内容 | 本文对应 |
|---|---|---|
| §1 | 模块定位与宿主边界 | 第 9、13、14 章 |
| §2 | 使用场景与非 SSO | 第 1、9、10 章 |
| §3 | 认证链路 | 第 9.2 节 |
| §4 | RP / User 配置 | 第 10、17 章 |
| §5 | Setup Mode | 第 11.1 节 |
| §6 | Setup Token | 第 11.2～11.3、17 章 |
| §7 | Passkey 注册 | 第 11.4～11.5 节 |
| §8 | 极简登录与系统认证界面 | 第 4.1、12.1～12.2 节 |
| §9 | 跨子域共享 | 第 10.3～10.6 节 |
| §10 | AuthStore 抽象 | 第 13、15 章 |
| §11 | 第二服务不需再次 Setup | 第 10.3、13.5、17.5 节 |
| §12 | Session 与 30 天 | 第 12.4～12.7 节 |
| §13 | Credential 数据 | 第 9.3、15.4 节 |
| §14 | WebAuthn 安全 | 第 10～12、18 章 |
| §15 | CLI / 管理接口 | 第 13、17 章 |
| §16 | SSH 恢复 | 第 11.6、19 章 |
| §17 | 模块接口 | 第 13 章 |
| §18 | HTTP 路由 | 第 16 章 |
| §19 | Challenge 存储 | 第 12.3 节 |
| §20 | 并发与多实例 | 第 13.3、14.2、15.6 节 |
| §21 | 默认 SQLiteAuthStore | 第 10.6、15、19 章 |
| §22 | 登录与 Setup 页面 | 第 4.1、11 章 |
| §23 | 错误处理 | 第 12、16.5、22 章 |
| §24 | Sign Counter | 第 12.8、15.4、22.3 节 |
| §25 | 浏览器与认证器兼容 | 第 12.2、20.3、22.4 节 |
| §26 | 安全、可复用、非独立服务原则 | 第 9、13、24 章 |
| §27 | 架构、类型、接口、实现与验收输出 | 第 9～24 章 |

### 25.2 本项目明确补齐、不是附件原文逐项规定的内容

| 补充项 | 决策 |
|---|---|
| 登录选项语义 | 一个可取消的 checkbox，默认不勾选 |
| 未勾选期限 | 12 小时服务端绝对期限＋会话 Cookie |
| 30 天续期 | 30 天绝对到期，不滑动续期 |
| Setup 跨进程 | 使用附件允许的摘要持久化方案，解决独立 CLI 与服务进程协作 |
| enabled 恢复 | Token 不自动绕过 disabled；运行配置变更必须显式生效 |
| 注册后登录 | 不自动签发业务 Session，返回正式登录页 |
| 可发现策略 | 新注册选择附件允许的 `residentKey=required`；登录使用固定单用户凭据列表以兼容既有凭据，另行验证可发现登录能力 |
| 默认 User ID | 32 随机字节；已有身份逐字节保留 |
| 宿主作用域 | Session / Token / Challenge 绑定 app_id 与 Origin |
| Cookie | Host-only、独立命名、无根域 Domain |
| 撤销影响 | 凭据撤销连带撤销该凭据派生的各宿主 Session |
| 敏感操作 | 增加 5 分钟内 Passkey 再认证要求 |
| 原子性 | 注册授权消费与保存凭据同一事务；登录与撤销竞态受保护 |
| 外部收藏入口 | Strict Cookie 下先加载无私人数据页面壳，再本源检查会话 |
| 备份与恢复 | 普通导出无认证材料；认证备份清除 Session / Setup；恢复显式确认 |
| 技术验证 | WebAuthn 库适配和实机兼容性必须先验证，不凭库名推断能力 |

---

<a id="section-26"></a>

## 26. 来源与技术参考

### A1：用户提供的认证规范

《Passkey / WebAuthn 认证模块开发提示词》，上传文件 `passkey_webauthn_auth_module_prompt(1).md`。本文按原文 §1～§27 建立覆盖表，引用位置已在正文标注。

### 官方技术参考

以下资料于本次文档编制时查阅，用于标准、构建和安全实现核对；依赖版本在实际工程验证后锁定。网址采用代码形式，便于离线 Markdown 保存和复制。

| 编号 | 官方资料 | 定位 |
|---|---|---|
| R1 | W3C Web Authentication Level 3 | RP / Origin、用户验证、可发现凭据和协议验证；`https://www.w3.org/TR/webauthn-3/` |
| R2 | MDN Set-Cookie | SameSite、会话恢复、Max-Age、Host-only 和 Cookie 前缀；`https://developer.mozilla.org/en-US/docs/Web/HTTP/Reference/Headers/Set-Cookie` |
| R3 | SQLite Write-Ahead Logging | WAL 与网络文件系统限制；`https://www.sqlite.org/wal.html` |
| R4 | webauthn-rs Webauthn API | 注册 / 认证与服务端配对状态；`https://docs.rs/webauthn-rs/latest/webauthn_rs/struct.Webauthn.html` |
| R5 | Rust include_bytes! | 编译期嵌入字节资源；`https://doc.rust-lang.org/std/macro.include_bytes.html` |
| R6 | rusqlite 项目与 API | bundled SQLite、备份及驱动能力；`https://github.com/rusqlite/rusqlite`；`https://docs.rs/rusqlite/latest/rusqlite/` |
| R7 | SQLite Online Backup API | 一致性在线数据库备份；`https://www.sqlite.org/backup.html` |
| R8 | SQLite FTS5 | tokenizer 与检索语义；`https://sqlite.org/fts5.html` |
| R9 | MDN share_target | PWA 分享接收能力及兼容性边界；`https://developer.mozilla.org/en-US/docs/Web/Progressive_web_apps/Manifest/Reference/share_target` |
| R10 | OWASP CSRF Prevention Cheat Sheet | Origin 与 CSRF 防护；`https://cheatsheetseries.owasp.org/cheatsheets/Cross-Site_Request_Forgery_Prevention_Cheat_Sheet.html` |
| R11 | OWASP SSRF Prevention Cheat Sheet | 主动抓取的地址、解析、重定向与网络边界；`https://cheatsheetseries.owasp.org/cheatsheets/Server_Side_Request_Forgery_Prevention_Cheat_Sheet.html` |
| R12 | Svelte Overview | 编译型前端组件；`https://svelte.dev/docs/svelte/overview` |
| R13 | Vite Building for Production | 静态生产构建；`https://vite.dev/guide/build` |
| R14 | webauthn-rs WebauthnBuilder | 精确 Origin 配置；`https://docs.rs/webauthn-rs/latest/webauthn_rs/struct.WebauthnBuilder.html` |
| R15 | webauthn-rs-core 文档 | 低层接口安全警告与安全封装边界；`https://docs.rs/webauthn-rs-core/latest/webauthn_rs_core/` |
| R16 | rust-openssl 文档 | vendored 构建说明；`https://docs.rs/openssl/latest/openssl/` |

---

**最终设计摘要：一个以 Passkey 为唯一正式登录方式、通过可复用认证子模块保护、以本地持久数据为中心的单用户收藏应用。首页服务于快速打开网站，添加入口服务于低摩擦保存，导入导出和备份保证数据可迁移。多个宿主可以共享 Passkey credential，但各自的 Session 和业务数据保持独立。**
