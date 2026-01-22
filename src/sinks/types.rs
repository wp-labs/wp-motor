use super::backends::file::FileSink;
use super::prelude::*;
use super::routing::agent::SinkGroupAgent;
// use super::sink_build::AsyncFileSinkEx; // removed after unifying to Proxy-only
// External sinks moved to extensions/sinks via wp-sink-api; core keeps only File and Proxy
use super::utils::buffer_monitor::BufferMonitor;
use super::utils::buffer_monitor::WatchOuterImpl;
use super::utils::formatter::FormatAdapter;
use super::utils::view::ViewOuter;
use async_trait::async_trait;
use derive_more::Deref;
use derive_more::Display;
use std::fmt::{Display, Formatter};
use std::sync::Arc;

use wp_model_core::model::DataRecord;

use crate::core::sinks::sync_sink::traits::SyncCtrl;
use crate::types::Abstract;

use wp_connector_api::SinkResult;

#[derive(Display)]
pub enum SinkBackendType {
    #[display("proxy")]
    Proxy(Box<dyn AsyncSink>),
}

pub use wp_connector_api::AsyncSink;

// Re-export MockSink from test_helpers for backward compatibility
#[cfg(test)]
pub use crate::sinks::test_helpers::MockSink;

pub struct ASinkHandle {
    pub name: String,
    pub sink: SinkBackendType,
}
impl ASinkHandle {
    pub fn new(name: String, sink: SinkBackendType) -> Self {
        Self { name, sink }
    }
}

pub type ASinkSender = tokio::sync::mpsc::Sender<ASinkHandle>;
pub type ASinkReceiver = tokio::sync::mpsc::Receiver<ASinkHandle>;

#[derive(Clone)]
pub enum SinkEndpoint {
    File(FormatAdapter<FileSink>),
    WFile(FormatAdapter<WatchOuterImpl<FileSink>>),
    Buffer(FormatAdapter<BufferMonitor>),
    View(ViewOuter),
    Null,
}

#[async_trait]
impl AsyncCtrl for SinkBackendType {
    async fn stop(&mut self) -> SinkResult<()> {
        match self {
            SinkBackendType::Proxy(f) => f.stop().await,
        }
    }

    async fn reconnect(&mut self) -> SinkResult<()> {
        match self {
            SinkBackendType::Proxy(f) => f.reconnect().await,
        }
    }
}

// SinkEndpoint 不应该直接实现 RecSyncSink
// 因为它包含不同类型的 endpoint，有些需要 RecSyncSink，有些需要 FFVSyncSink

impl SyncCtrl for SinkEndpoint {
    fn stop(&mut self) -> SinkResult<()> {
        match self {
            SinkEndpoint::File(f) => f.stop(),
            SinkEndpoint::WFile(f) => f.stop(),
            SinkEndpoint::Buffer(b) => b.stop(),
            SinkEndpoint::View(b) => b.stop(),
            _ => Ok(()),
        }
    }
}

#[async_trait]
impl AsyncRecordSink for SinkBackendType {
    async fn sink_record(&mut self, data: &DataRecord) -> SinkResult<()> {
        match self {
            SinkBackendType::Proxy(f) => f.sink_record(data).await,
        }
    }

    async fn sink_records(&mut self, data: Vec<Arc<DataRecord>>) -> SinkResult<()> {
        match self {
            SinkBackendType::Proxy(f) => f.sink_records(data).await,
        }
    }
}

#[async_trait]
impl AsyncRawdatSink for SinkBackendType {
    async fn sink_bytes(&mut self, data: &[u8]) -> SinkResult<()> {
        match self {
            SinkBackendType::Proxy(f) => f.sink_bytes(data).await,
        }
    }
    async fn sink_str(&mut self, data: &str) -> SinkResult<()> {
        match self {
            SinkBackendType::Proxy(f) => f.sink_str(data).await,
        }
    }

    async fn sink_str_batch(&mut self, data: Vec<&str>) -> SinkResult<()> {
        match self {
            SinkBackendType::Proxy(f) => f.sink_str_batch(data).await,
        }
    }

    async fn sink_bytes_batch(&mut self, data: Vec<&[u8]>) -> SinkResult<()> {
        match self {
            SinkBackendType::Proxy(f) => f.sink_bytes_batch(data).await,
        }
    }
}

pub trait SinkInfraAble {
    fn miss(&self) -> SinkGroupAgent;
    fn error(&self) -> SinkGroupAgent;
    fn residue(&self) -> SinkGroupAgent;
}

pub trait SinkRouteAble {
    fn route(&self, target_rule: &str) -> Option<SinkGroupAgent>;
}

#[derive(Clone, Debug)]
pub enum ProcMeta {
    Rule(String),
    Null,
}

impl ProcMeta {
    pub fn abstract_info(&self) -> String {
        match self {
            ProcMeta::Rule(s) => format!("rule:{}", s),
            ProcMeta::Null => "null".to_string(),
        }
    }
}
#[derive(Clone, Debug)]
pub enum SinkDataEnum {
    Rec(ProcMeta, Arc<DataRecord>),
    FFV(FmtFieldVec),
    Raw(String),
}

#[derive(Clone, Debug, getset::Getters)]
#[get = "pub"]
pub struct SinkRecUnit {
    id: PkgID,
    meta: ProcMeta,
    data: Arc<DataRecord>,
}

impl SinkRecUnit {
    /// Create a new SinkRecUnit with the given id, meta, and data
    pub fn new(id: PkgID, meta: ProcMeta, data: Arc<DataRecord>) -> Self {
        Self { id, meta, data }
    }

    /// Create a SinkRecUnit with record data
    pub fn with_record(id: PkgID, meta: ProcMeta, record: Arc<DataRecord>) -> Self {
        Self {
            id,
            meta,
            data: record,
        }
    }

    /// Consume the unit and return its raw components
    pub fn into_parts(self) -> (PkgID, ProcMeta, Arc<DataRecord>) {
        let Self { id, meta, data } = self;
        (id, meta, data)
    }
}

#[derive(Clone, Debug, getset::Getters)]
#[get = "pub"]
pub struct SinkFFVUnit {
    id: PkgID,
    data: FmtFieldVec,
}

impl SinkFFVUnit {
    /// Create a new SinkFFVUnit with the given id and data
    pub fn new(id: PkgID, data: FmtFieldVec) -> Self {
        Self { id, data }
    }
}

#[derive(Clone, Debug, getset::Getters)]
#[get = "pub"]
pub struct SinkStrUnit {
    id: PkgID,
    data: String,
}

impl SinkStrUnit {
    /// Create a new SinkStrUnit with the given id and data
    pub fn new(id: PkgID, data: String) -> Self {
        Self { id, data }
    }
}

#[derive(Clone, Debug, getset::Getters)]
#[get = "pub"]
pub struct SinkDataUnit {
    id: PkgID,
    data: SinkDataEnum,
}

impl SinkDataUnit {
    /// 创建一个新的 SinkDataUnit
    pub fn new(id: PkgID, data: SinkDataEnum) -> Self {
        Self { id, data }
    }

    /// 创建包含记录数据的 SinkDataUnit
    pub fn with_record(id: PkgID, meta: ProcMeta, record: Arc<DataRecord>) -> Self {
        Self {
            id,
            data: SinkDataEnum::Rec(meta, record),
        }
    }

    /// 创建包含字段向量数据的 SinkDataUnit
    pub fn with_ffv(id: PkgID, ffv: FmtFieldVec) -> Self {
        Self {
            id,
            data: SinkDataEnum::FFV(ffv),
        }
    }

    /// 创建包含原始字符串数据的 SinkDataUnit
    pub fn with_raw(id: PkgID, raw: String) -> Self {
        Self {
            id,
            data: SinkDataEnum::Raw(raw),
        }
    }
}

#[derive(Clone, Debug, Deref)]
pub struct SinkPackage {
    items: Vec<SinkRecUnit>,
}

#[derive(Clone, Debug, Deref)]
pub struct SinkFFVPackage {
    items: Vec<SinkFFVUnit>,
}

#[derive(Clone, Debug, Deref)]
pub struct SinkStrPackage {
    items: Vec<SinkStrUnit>,
}

impl SinkPackage {
    /// 创建一个空的 SinkPackage
    pub fn new() -> Self {
        Self { items: Vec::new() }
    }

    /// 创建包含单个元素的 SinkPackage
    pub fn single(unit: SinkRecUnit) -> Self {
        Self { items: vec![unit] }
    }

    /// 添加一个 SinkDataUnit
    pub fn push(&mut self, unit: SinkRecUnit) {
        self.items.push(unit);
    }

    /// 从迭代器创建 SinkPackage
    pub fn from_units<I>(iter: I) -> Self
    where
        I: IntoIterator<Item = SinkRecUnit>,
    {
        Self {
            items: iter.into_iter().collect(),
        }
    }

    /// 获取元素数量
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// 检查是否为空
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// 清空所有元素
    pub fn clear(&mut self) {
        self.items.clear();
    }

    /// 迭代访问所有元素
    pub fn iter(&self) -> std::slice::Iter<'_, SinkRecUnit> {
        self.items.iter()
    }

    /// 可变迭代访问所有元素
    pub fn iter_mut(&mut self) -> std::slice::IterMut<'_, SinkRecUnit> {
        self.items.iter_mut()
    }

    pub fn into_inner(self) -> Vec<SinkRecUnit> {
        self.items
    }

    /// 批量更新所有元素的元数据
    pub fn update_meta(&mut self, new_meta: ProcMeta) {
        for unit in &mut self.items {
            // 由于没有 setter，我们需要创建新的单元
            let id = *unit.id();
            let data = unit.data().clone();
            *unit = SinkRecUnit::new(id, new_meta.clone(), data);
        }
    }

    /// 批量更新所有元素的元数据（使用函数）
    pub fn update_meta_with<F>(&mut self, updater: F)
    where
        F: Fn(&ProcMeta) -> ProcMeta,
    {
        for unit in &mut self.items {
            let id = *unit.id();
            let data = unit.data().clone();
            let new_meta = updater(unit.meta());
            *unit = SinkRecUnit::new(id, new_meta, data);
        }
    }

    /// 提取规则信息
    pub fn extract_rule(data: &SinkRecUnit) -> ProcMeta {
        data.meta().clone()
    }
}

impl Default for SinkPackage {
    fn default() -> Self {
        Self::new()
    }
}

impl FromIterator<SinkRecUnit> for SinkPackage {
    fn from_iter<I>(iter: I) -> Self
    where
        I: IntoIterator<Item = SinkRecUnit>,
    {
        Self {
            items: iter.into_iter().collect(),
        }
    }
}

impl IntoIterator for SinkPackage {
    type Item = SinkRecUnit;
    type IntoIter = std::vec::IntoIter<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        self.items.into_iter()
    }
}

impl<'a> IntoIterator for &'a SinkPackage {
    type Item = &'a SinkRecUnit;
    type IntoIter = std::slice::Iter<'a, SinkRecUnit>;

    fn into_iter(self) -> Self::IntoIter {
        self.items.iter()
    }
}

impl Abstract for SinkDataEnum {
    fn abstract_info(&self) -> String {
        match self {
            SinkDataEnum::Rec(_, tdc) => format!("sink_data::tdc{}", tdc),
            SinkDataEnum::FFV(ffv) => {
                format!("sink_data::ffv len:{}", ffv.len())
            }
            SinkDataEnum::Raw(raw) => {
                let len = if raw.len() > 10 { 10 } else { raw.len() };
                format!("sink_data::raw{}", raw.split_at(len).0)
            }
        }
    }
}
/*
impl From<TDOCrate> for SinkDataEnum {
    fn from(tdc: TDOCrate) -> Self {
        SinkDataEnum::TDC(tdc)
    }
}

 */
impl From<FmtFieldVec> for SinkDataEnum {
    fn from(ffv: FmtFieldVec) -> Self {
        SinkDataEnum::FFV(ffv)
    }
}
impl From<String> for SinkDataEnum {
    fn from(raw: String) -> Self {
        SinkDataEnum::Raw(raw)
    }
}

impl Display for SinkDataEnum {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            SinkDataEnum::Rec(_, _) => write!(f, "tdc"),
            SinkDataEnum::FFV(_) => write!(f, "ffv"),
            SinkDataEnum::Raw(_) => write!(f, "raw"),
        }
    }
}

pub type SinkPkgSender = tokio::sync::mpsc::Sender<SinkPackage>;
pub type SinkPkgReceiver = tokio::sync::mpsc::Receiver<SinkPackage>;

pub type SinkFFVSender = tokio::sync::mpsc::Sender<SinkFFVPackage>;
pub type SinkFFVReceiver = tokio::sync::mpsc::Receiver<SinkFFVPackage>;

pub type SinkStrSender = tokio::sync::mpsc::Sender<SinkStrPackage>;
pub type SinkStrReceiver = tokio::sync::mpsc::Receiver<SinkStrPackage>;

pub type SinkDatASender = SinkPkgSender;
pub type SinkDatAReceiver = SinkPkgReceiver;

// 事件化：使用 tokio::mpsc 作为分发通道，便于与 runtime 协作
pub type SinkDatYSender = SinkDatASender;
pub type SinkDatYReceiver = SinkDatAReceiver;

// 注：旧实现使用 std::sync::mpsc::SyncSender 以阻塞语义提供背压，
// 迁移后在调用侧使用 `blocking_send` 达到一致的背压效果；
// 同时在消费者侧可以自然地用 `recv().await/try_recv()` 事件化处理。

// SinkFFVPackage implementations
impl SinkFFVPackage {
    /// 创建一个空的 SinkFFVPackage
    pub fn new() -> Self {
        Self { items: Vec::new() }
    }

    /// 创建包含单个元素的 SinkFFVPackage
    pub fn single(unit: SinkFFVUnit) -> Self {
        Self { items: vec![unit] }
    }

    /// 添加一个 SinkFFVUnit
    pub fn push(&mut self, unit: SinkFFVUnit) {
        self.items.push(unit);
    }

    /// 从迭代器创建 SinkFFVPackage
    pub fn from_units<I>(iter: I) -> Self
    where
        I: IntoIterator<Item = SinkFFVUnit>,
    {
        Self {
            items: iter.into_iter().collect(),
        }
    }

    /// 获取元素数量
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// 检查是否为空
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// 清空所有元素
    pub fn clear(&mut self) {
        self.items.clear();
    }

    /// 迭代访问所有元素
    pub fn iter(&self) -> std::slice::Iter<'_, SinkFFVUnit> {
        self.items.iter()
    }

    /// 可变迭代访问所有元素
    pub fn iter_mut(&mut self) -> std::slice::IterMut<'_, SinkFFVUnit> {
        self.items.iter_mut()
    }
}

impl Default for SinkFFVPackage {
    fn default() -> Self {
        Self::new()
    }
}

impl FromIterator<SinkFFVUnit> for SinkFFVPackage {
    fn from_iter<I>(iter: I) -> Self
    where
        I: IntoIterator<Item = SinkFFVUnit>,
    {
        Self {
            items: iter.into_iter().collect(),
        }
    }
}

impl IntoIterator for SinkFFVPackage {
    type Item = SinkFFVUnit;
    type IntoIter = std::vec::IntoIter<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        self.items.into_iter()
    }
}

impl<'a> IntoIterator for &'a SinkFFVPackage {
    type Item = &'a SinkFFVUnit;
    type IntoIter = std::slice::Iter<'a, SinkFFVUnit>;

    fn into_iter(self) -> Self::IntoIter {
        self.items.iter()
    }
}

// SinkStrPackage implementations
impl SinkStrPackage {
    /// 创建一个空的 SinkStrPackage
    pub fn new() -> Self {
        Self { items: Vec::new() }
    }

    /// 创建包含单个元素的 SinkStrPackage
    pub fn single(unit: SinkStrUnit) -> Self {
        Self { items: vec![unit] }
    }

    /// 添加一个 SinkStrUnit
    pub fn push(&mut self, unit: SinkStrUnit) {
        self.items.push(unit);
    }

    /// 从迭代器创建 SinkStrPackage
    pub fn from_units<I>(iter: I) -> Self
    where
        I: IntoIterator<Item = SinkStrUnit>,
    {
        Self {
            items: iter.into_iter().collect(),
        }
    }

    /// 获取元素数量
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// 检查是否为空
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// 清空所有元素
    pub fn clear(&mut self) {
        self.items.clear();
    }

    /// 迭代访问所有元素
    pub fn iter(&self) -> std::slice::Iter<'_, SinkStrUnit> {
        self.items.iter()
    }

    /// 可变迭代访问所有元素
    pub fn iter_mut(&mut self) -> std::slice::IterMut<'_, SinkStrUnit> {
        self.items.iter_mut()
    }
}

impl Default for SinkStrPackage {
    fn default() -> Self {
        Self::new()
    }
}

impl FromIterator<SinkStrUnit> for SinkStrPackage {
    fn from_iter<I>(iter: I) -> Self
    where
        I: IntoIterator<Item = SinkStrUnit>,
    {
        Self {
            items: iter.into_iter().collect(),
        }
    }
}

impl IntoIterator for SinkStrPackage {
    type Item = SinkStrUnit;
    type IntoIter = std::vec::IntoIter<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        self.items.into_iter()
    }
}

impl<'a> IntoIterator for &'a SinkStrPackage {
    type Item = &'a SinkStrUnit;
    type IntoIter = std::slice::Iter<'a, SinkStrUnit>;

    fn into_iter(self) -> Self::IntoIter {
        self.items.iter()
    }
}
