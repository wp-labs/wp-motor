use contracts::debug_requires;
use serde_derive::{Deserialize, Serialize};
use std::fmt::{Debug, Display, Formatter};
use strum_macros::Display;
use wp_model_core::model::DataRecord;

use crate::traits::{Mergeable, SliceMetrics, SlicesMetadata};

use super::{dimension::DataDim, measure::MeasureUnit};

pub trait SliceItem: Debug + Clone + Default + SlicesMetadata {}

impl<T> SliceItem for T where T: Debug + Clone + Default + SlicesMetadata {}

#[derive(Debug, PartialEq, Deserialize, Serialize, Clone, Default, Display)]
pub enum StatStage {
    #[serde(rename = "gen")]
    Gen,
    #[serde(rename = "pick")]
    #[default]
    Pick,
    #[serde(rename = "parse")]
    Parse,
    #[serde(rename = "sink")]
    Sink,
}

#[derive(Clone, Debug)]
pub struct SliceRecord<T>
where
    T: SliceItem,
{
    pub stage: StatStage,
    item_key: String,
    value: DataDim,
    pub stat: MeasureUnit,
    _phantom: std::marker::PhantomData<T>,
}

impl<T> SliceRecord<T>
where
    T: SliceItem,
{
    #[debug_requires(! item_key.is_empty(), "name is empty")]
    pub fn new(stage: StatStage, item_key: String, value: DataDim) -> Self {
        Self {
            item_key,
            value,
            stage,
            stat: MeasureUnit::new(),
            _phantom: Default::default(),
        }
    }
    #[debug_requires(! name.is_empty(), "name is empty")]
    pub fn update_name(&mut self, name: String) {
        self.item_key = name;
    }
    pub fn can_merge(&self, other: &Self) -> bool {
        self.stage == other.stage && self.item_key == other.item_key
    }
    pub fn covert_tdc(&self, v_reqs: &[String]) -> DataRecord {
        let mut tdc = self.value.to_tdc(v_reqs);
        tdc.append(&mut self.stat.to_tdc());
        DataRecord::from(tdc)
    }
}

impl<T> Mergeable<SliceRecord<T>> for SliceRecord<T>
where
    T: SliceItem,
{
    fn merge(&mut self, other: SliceRecord<T>) {
        self.stat.add(&other.stat);
    }
}

impl<T> SliceMetrics for SliceRecord<T>
where
    T: SliceItem,
{
    #[debug_requires(! self.item_key.is_empty(), "name is empty")]
    fn slices_key(&self) -> &str {
        self.item_key.as_str()
    }

    fn add(&mut self, other: &Self) {
        self.stat.add(&other.stat);
    }

    fn rec_in(&mut self) {
        self.stat.rec_in();
    }

    fn rec_suc(&mut self) {
        self.stat.rec_suc();
    }
    fn rec_end(&mut self) {
        self.stat.rec_end();
    }

    fn get_total(&self) -> u64 {
        self.stat.get_total()
    }
}

impl<T> SliceRecord<T>
where
    T: SliceItem,
{
    pub fn rec_beg(&mut self) {
        self.stat.rec_in();
    }
    pub fn rec_end(&mut self) {
        self.stat.rec_suc();
    }
    pub fn rec_beg_end(&mut self) {
        self.stat.rec_in();
        self.stat.rec_suc();
    }
    pub fn rec_beg_end_n(&mut self, n: usize) {
        self.stat.rec_beg_end_n(n);
    }
    pub fn get_value(&self) -> &DataDim {
        &self.value
    }
}

impl<T> Display for SliceRecord<T>
where
    T: SliceItem,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "{:<48}{} ", self.item_key, self.stat)?;
        Ok(())
    }
}
