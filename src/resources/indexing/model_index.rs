use std::{
    collections::HashMap,
    fmt::{Display, Formatter},
};

use super::{IndexDisplay, ModelName, ModelNameSet};
use crate::resources::SinkID;

//use super::rule_index::{IndexDisplay, ModelName, ModelNameSet, SinkID};

#[derive(Default)]
pub struct SinkModelIndex(HashMap<SinkID, ModelNameSet>);
impl SinkModelIndex {
    pub fn associate_model(&mut self, sink_name: &SinkID, mdl_name: ModelName) {
        if let Some(sink_mdls) = self.0.get_mut(sink_name) {
            sink_mdls.insert(mdl_name.clone());
        } else {
            let mut sink_mdls = ModelNameSet::default();
            sink_mdls.insert(mdl_name.clone());
            self.0.insert(sink_name.clone(), sink_mdls);
        }
    }

    pub fn disassociate_mdl(&mut self, sink_name: &SinkID, mdl_name: &ModelName) {
        if let Some(sink_mdls) = self.0.get_mut(sink_name) {
            sink_mdls.remove(mdl_name);
        }
    }
    pub fn get(&self, sink_name: &SinkID) -> Option<&ModelNameSet> {
        self.0.get(sink_name)
    }
}
impl Display for SinkModelIndex {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        for (k, v) in &self.0 {
            writeln!(f, "{:<50} : {} ", k, IndexDisplay::new(v))?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use crate::resources::{ModelName, SinkID};

    use super::SinkModelIndex;

    #[test]
    fn test_sinkdmls() {
        let mut idx = SinkModelIndex::default();
        let sink = SinkID("sink".into());
        let mdl1 = ModelName("mdl1".into());
        idx.associate_model(&sink, mdl1.clone());
        assert_eq!(idx.get(&sink).and_then(|x| x.get(&mdl1)), Some(&mdl1));
        idx.disassociate_mdl(&sink, &mdl1);
        assert_eq!(idx.get(&sink).and_then(|x| x.get(&mdl1)), None);
    }
}
