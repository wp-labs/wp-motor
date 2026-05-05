use super::super::prelude::*;
use crate::compat::LegacyOwe;
use oml::parser::code::OMLCode;
use orion_conf::EnvTomlLoad;
use orion_error::{UnifiedReason, runtime::WithContext};
use orion_variate::EnvDict;
use std::{
    collections::{HashMap, VecDeque},
    fs::File,
    io::Read,
};
use wp_conf::{
    paths::{GEN_FIELD_FILE, GEN_RULE_FILE},
    utils::{find_conf_files, find_group_conf},
};
use wp_error::{
    config_error::{ConfError, ConfReason, ConfResult},
    parse_error::{OMLCodeReason, OMLCodeResult},
};
use wp_model_core::model::DataType;
use wpl::{
    ParserFactory, WplCode, WplPackage, WplRule, WplSep, WplStatementType,
    generator::{FieldsGenRule, FmtFieldVec, GenChannel, NamedFieldGF},
};

use crate::{resources::OmlRepository, stat::MonSend};
use wp_log::info_ctrl;

#[derive(Clone)]
pub struct GenRuleUnit {
    package: WplPackage,
    fields: NamedFieldGF,
}
impl GenRuleUnit {
    pub fn new(package: WplPackage, fields: NamedFieldGF) -> Self {
        GenRuleUnit { package, fields }
    }
    pub fn get_rules(&self) -> &VecDeque<WplRule> {
        &self.package.rules
    }
    pub fn get_fields(&self) -> &NamedFieldGF {
        &self.fields
    }
    pub fn is_empty(&self) -> bool {
        self.package.is_empty()
    }
    pub async fn send_stat(&mut self, _mon_s: &MonSend) -> ConfResult<()> {
        //roll queue to send stat
        let len = self.package.rules.len();
        for _ in 0..len {
            if let Some(rule) = self.package.rules.pop_front() {
                //let snap = rule.stat.borrow_mut().swap_snap();
                //mon_s.send(StatSlices::Gen(snap)).await?;
                self.package.rules.push_back(rule);
            }
        }
        Ok(())
    }
    pub fn generat(&mut self) -> ConfResult<Vec<FmtFieldVec>> {
        let mut result = Vec::new();
        if self.get_rules().is_empty() {
            return Err(ConfError::from(ConfReason::NotFound(
                "rule unit is empty".into(),
            )));
        }
        let ups_sep = WplSep::default();
        for wpl_rule in self.get_rules() {
            let mut fieldset = FmtFieldVec::new();
            let WplStatementType::Express(rule) = &wpl_rule.statement;
            for group in &rule.group {
                for f_conf in &group.fields {
                    let rule = f_conf
                        .name
                        .as_ref()
                        .and_then(|name| self.get_fields().get(name));
                    let mut ch = GenChannel::new();
                    let meta = DataType::from(f_conf.meta_name.as_str()).map_err(|e| {
                        ConfError::from(ConfReason::Syntax(format!(
                            "invalid field meta '{}': {}",
                            f_conf.meta_name, e
                        )))
                    })?;
                    let parser = ParserFactory::create(&meta).map_err(|e| {
                        ConfError::from(ConfReason::Syntax(format!(
                            "create parser for meta '{}' failed: {}",
                            f_conf.meta_name, e
                        )))
                    })?;
                    let sep = group.resolve_sep(&ups_sep);
                    let field = parser.generate(&mut ch, &sep, f_conf, rule).map_err(|e| {
                        ConfError::from(ConfReason::Syntax(format!(
                            "generate field '{}' failed: {}",
                            f_conf.name.as_deref().unwrap_or(f_conf.meta_name.as_str()),
                            e
                        )))
                    })?;
                    fieldset.push(field);
                }
            }
            result.push(fieldset);
        }
        Ok(result)
    }
}
pub fn load_gen_confs(path: &str, dict: &EnvDict) -> ConfResult<Vec<GenRuleUnit>> {
    let files = find_group_conf(path, GEN_RULE_FILE, GEN_FIELD_FILE)?;
    if files.is_empty() {
        return Err(ConfError::from(ConfReason::NotFound(String::new()))
            .with_detail("gen rule conf file is empty"));
    }

    let mut result_vec = Vec::new();
    for f in files {
        let mut package_opt = None;
        if let Some(fst) = &f.fst {
            let mut ctx = WithContext::doing("load gen code");
            ctx.record("fst", fst.to_str().unwrap_or("unknow"));
            let mut f = File::open(fst)
                .owe(ConfReason::NotFound("open file fail!".into()))
                .with_context(&ctx)?;
            let mut buffer = Vec::with_capacity(10240);
            f.read_to_end(&mut buffer).expect("read conf file error");
            let data = String::from_utf8(buffer).expect("conf file is not utf8");
            let code_build = WplCode::build(fst.clone(), data.as_str())
                .owe(ConfReason::Uvs(UnifiedReason::core_conf()))
                .with_context(&ctx)?;
            info_ctrl!("load conf file: {:?}", fst);
            let package = code_build
                .parse_pkg()
                .owe(ConfReason::Uvs(UnifiedReason::core_conf()))
                .with_context(&ctx)?;
            if package.is_empty() {
                return Err(ConfError::from(ConfReason::NotFound(
                    "gen rule package is empty".into(),
                )));
            }
            package_opt = Some(package);
        }
        let mut fields = HashMap::new();
        if let Some(sec) = &f.sec {
            let mut ctx = WithContext::doing("loadd field gen rule");
            ctx.record("sec", sec.to_str().unwrap_or("unknow"));
            let toml = std::fs::read_to_string(sec)
                .owe(ConfReason::Uvs(UnifiedReason::core_conf()))
                .with_context(&ctx)?;
            let conf: FieldsGenRule = FieldsGenRule::env_parse_toml(toml.as_str(), dict)
                .owe(ConfReason::Uvs(UnifiedReason::core_conf()))
                .with_context(&ctx)?;
            fields = conf.items;
            info_ctrl!("load conf file: {:?}", sec);
        }
        if let Some(packages) = package_opt {
            result_vec.push(GenRuleUnit::new(packages, fields.clone()));
        }
    }
    Ok(result_vec)
}

pub fn fetch_oml_data(path: &str, target: &str) -> OMLCodeResult<OmlRepository> {
    let mut ctx = WithContext::doing("load oml");
    ctx.record("path", path);
    let files = find_conf_files(path, target)
        .owe(OMLCodeReason::Uvs(UnifiedReason::core_conf()))
        .with_context(&ctx)?;

    let mut spc = OmlRepository::default();
    for f_name in &files {
        info_ctrl!("load conf file: {:?}", f_name);
        let mut f = File::open(f_name)
            .owe(OMLCodeReason::Uvs(UnifiedReason::core_conf()))
            .with_context(&ctx)?;
        let mut buffer = Vec::with_capacity(10240);
        f.read_to_end(&mut buffer)
            .owe(OMLCodeReason::Uvs(UnifiedReason::core_conf()))
            .with_context(&ctx)?;
        let file_data = String::from_utf8(buffer)
            .owe(OMLCodeReason::Uvs(UnifiedReason::core_conf()))
            .with_context(&ctx)?;
        spc.push(OMLCode::from((
            f_name.to_str().unwrap_or("").to_string(),
            file_data,
        )))
    }
    Ok(spc)
}
