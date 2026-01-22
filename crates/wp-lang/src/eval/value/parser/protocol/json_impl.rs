use super::super::prelude::*;
use crate::ast::WplSep;
use crate::eval::runtime::field::FieldEvalUnit;
use smol_str::SmolStr;
use wp_model_core::model::FNameStr;
// 使用 String 动态拼接路径，避免固定容量 ArrayString 在深层或长 key 时 panic
use serde_json::{Map, Value};
use wp_model_core::model::types::value::ObjectValue;
use wp_model_core::model::{DataField, DataType};

pub struct JsonProc {}

impl JsonProc {
    // 最大嵌套深度，防止极端输入导致栈溢出
    const MAX_DEPTH: usize = 128;
    #[inline]
    fn max_depth(fpu: &FieldEvalUnit) -> usize {
        if let Some(cnt) = fpu.conf().field_cnt() {
            cnt
        } else if let Some(len) = fpu.conf().length() {
            *len
        } else {
            Self::MAX_DEPTH
        }
    }
    #[inline]
    fn json_path(parent: &str, cur: &str) -> String {
        if parent.is_empty() {
            return cur.to_string();
        }
        // 预估容量，避免频繁扩容；但不设置硬上限，保证不会因超长 key/深层路径而 panic
        let mut path = String::with_capacity(parent.len() + 1 + cur.len());
        path.push_str(parent);
        path.push('/');
        path.push_str(cur);
        path
    }

    #[inline]
    fn string_preserve_escapes(v: &Value) -> String {
        let raw = v.to_string();
        let trimmed = raw.trim();
        let without_prefix = trimmed.strip_prefix('"').unwrap_or(trimmed);
        let without_suffix = without_prefix.strip_suffix('"').unwrap_or(without_prefix);
        without_suffix.to_string()
    }

    #[allow(clippy::too_many_arguments)]
    fn proc_json_map(
        fpu: &FieldEvalUnit,
        upper_sep: &WplSep,
        p_path: &str,
        v_map: &Map<String, Value>,
        _name: &str,
        exact: bool,
        out: &mut Vec<DataField>,
        depth: usize,
        max_depth: usize,
    ) -> ModalResult<()> {
        //let mut sub_fields = Vec::with_capacity(20);
        for (k, v) in v_map {
            Self::proc_value_inner(
                fpu,
                upper_sep,
                p_path,
                v,
                k,
                exact,
                out,
                depth + 1,
                max_depth,
            )?;
        }
        // 深层缺失字段检测（按层）：仅检查当前对象的直接子级中，配置为必选且非通配的字段
        if exact && let Some(subs) = fpu.conf().sub_fields.as_ref() {
            let mut prefix = String::new();
            if !p_path.is_empty() {
                prefix.push_str(p_path);
                prefix.push('/');
            }
            let conf_items = subs.conf_items();
            let mut required_children: Vec<String> = Vec::new();
            for (k, conf) in conf_items.exact_iter() {
                if *conf.is_opt() {
                    continue;
                }
                if let Some(rest) = k.strip_prefix(&prefix)
                    && !rest.is_empty()
                    && let Some(child) = rest.split('/').next()
                    && !rest.contains('/')
                    && !required_children.iter().any(|c| c == child)
                {
                    required_children.push(child.to_string());
                }
            }
            if !required_children.is_empty() {
                let mut missing: Vec<String> = Vec::new();
                for rc in required_children {
                    if !v_map.contains_key(&rc) {
                        missing.push(rc);
                    }
                }
                if !missing.is_empty() {
                    let mut bad = "";
                    let missing_str = if p_path.is_empty() {
                        missing.join(", ")
                    } else {
                        format!("{}/{{{}}}", p_path, missing.join(", "))
                    };
                    let _: &str = alt((missing_str.as_str(), "<exact_json missing>"))
                        .context(ctx_desc("exact_json missing fields"))
                        .parse_next(&mut bad)?;
                    unreachable!("exact_json missing fields should not succeed");
                }
            }
        }
        Ok(())
    }
    fn exact_check(
        fpu: &FieldEvalUnit,
        exact: bool,
        have_conf: bool,
        conf_path: &str,
    ) -> ModalResult<()> {
        if exact && !have_conf {
            // 将不匹配的字段路径与允许集合一起放入“期望”中，便于报错阅读
            let allow_list = fpu
                .conf()
                .sub_fields
                .as_ref()
                .map(|subs| subs.to_string())
                .unwrap_or_else(|| "<empty>".to_string());

            let mut bad = "";
            // 通过期望 conf_path 与 allowed 集合同时构造错误上下文
            let _: &str = alt((conf_path, allow_list.as_str()))
                .context(ctx_desc("exact_json mismatch"))
                .parse_next(&mut bad)?;
            unreachable!("alt(conf_path, allowed) on empty input should never succeed");
        }
        Ok(())
    }

    pub(crate) fn proc_value(
        fpu: &FieldEvalUnit,
        upper_sep: &WplSep,
        parent: &str,
        v: &Value,
        name: &str,
        exact: bool,
        out: &mut Vec<DataField>,
    ) -> ModalResult<()> {
        let max_depth = Self::max_depth(fpu);
        Self::proc_value_inner(fpu, upper_sep, parent, v, name, exact, out, 0, max_depth)
    }

    #[allow(clippy::too_many_arguments)]
    fn proc_value_inner(
        fpu: &FieldEvalUnit,
        upper_sep: &WplSep,
        parent: &str,
        v: &Value,
        name: &str,
        exact: bool,
        out: &mut Vec<DataField>,
        depth: usize,
        max_depth: usize,
    ) -> ModalResult<()> {
        if depth > max_depth {
            return fail
                .context(ctx_desc("json nested too deep"))
                .parse_next(&mut "");
        }
        let j_path = Self::json_path(parent, name);
        // 无子配置时走快路径，避免不必要的哈希检索
        let has_subs = fpu.conf().sub_fields().is_some();
        let sub_conf_opt = if has_subs {
            fpu.conf()
                .sub_fields
                .as_ref()
                .and_then(|subs| subs.get(&j_path))
        } else {
            None
        };
        let run_key = if let Some(sub_conf) = sub_conf_opt {
            sub_conf.run_key_str(j_path.as_str())
        } else {
            FNameStr::from(j_path.clone())
        };
        match v {
            Value::Null => {
                //no need process
            }
            Value::Bool(b_v) => {
                if exact {
                    Self::exact_check(fpu, true, sub_conf_opt.is_some(), j_path.as_str())?;
                }
                if let Some(_cur_conf) = sub_conf_opt {
                    let dat_str = format!("{}", b_v);
                    if let Some(fpu) = fpu.get_sub_fpu(j_path.as_str()) {
                        let ups_sep = fpu.conf().resolve_sep(upper_sep);
                        fpu.parse(&ups_sep, &mut dat_str.as_str(), Some(run_key), out)?;
                        return Ok(());
                    }
                }
                let field = DataField::from_bool(run_key, *b_v);
                out.push(field);
            }
            Value::Number(num) => {
                if exact {
                    Self::exact_check(fpu, true, sub_conf_opt.is_some(), j_path.as_str())?;
                }
                if let Some(_cur_conf) = sub_conf_opt {
                    let dat_str = format!("{}", num);
                    if let Some(fpu) = fpu.get_sub_fpu(j_path.as_str()) {
                        let ups_sep = fpu.conf().resolve_sep(upper_sep);
                        fpu.parse(&ups_sep, &mut dat_str.as_str(), Some(run_key), out)?;
                        return Ok(());
                    }
                }
                // 统一数值处理：优先 f64；其次 i64；u64 超出 i64 上限时降级为字符串，避免静默丢弃
                if let (true, Some(f)) = (num.is_f64(), num.as_f64()) {
                    out.push(DataField::from_float(run_key, f));
                } else if num.is_i64() {
                    if let Some(i_n) = num.as_i64() {
                        out.push(DataField::from_digit(run_key, i_n));
                    }
                } else if num.is_u64() {
                    if let Some(u) = num.as_u64() {
                        if u <= i64::MAX as u64 {
                            out.push(DataField::from_digit(run_key, u as i64));
                        } else {
                            // 超范围：保留精确性，降级为字符串
                            out.push(DataField::from_chars(
                                run_key,
                                SmolStr::from(num.to_string()),
                            ));
                        }
                    }
                } else {
                    // 兜底：未知数值类型，按字符串保留
                    out.push(DataField::from_chars(
                        run_key,
                        SmolStr::from(num.to_string()),
                    ));
                }
            }
            Value::String(_) => {
                let raw = Self::string_preserve_escapes(v);
                if exact {
                    Self::exact_check(fpu, true, sub_conf_opt.is_some(), j_path.as_str())?;
                }
                // Value::String(v_str) v_str会将‘\’符号吞，通过v.to_string()拿到日志原始数据
                if let Some(_cur_conf) = sub_conf_opt
                    && let Some(fpu) = fpu.get_sub_fpu(j_path.as_str())
                {
                    let mut ups_sep = fpu.conf().resolve_sep(upper_sep);
                    if ups_sep.is_space_sep() {
                        ups_sep.set_current("\\0")
                    }

                    let mut raw_ref = raw.as_str();
                    fpu.parse(&ups_sep, &mut raw_ref, Some(run_key), out)?;
                    return Ok(());
                }
                out.push(DataField::from_chars(run_key, raw));
                return Ok(());
            }
            Value::Array(arr) => {
                if exact {
                    Self::exact_check(fpu, true, sub_conf_opt.is_some(), j_path.as_str())?;
                }
                let mut arr_name = FNameStr::from(name);
                if let Some(cnf) = sub_conf_opt {
                    arr_name = cnf.name.clone().unwrap_or(arr_name);
                    if let DataType::Array(_) = *cnf.meta_type() {
                        let mut dat = Vec::with_capacity(arr.len());
                        if fpu.get_sub_fpu(j_path.as_str()).is_some() {
                            for v in arr.iter() {
                                if let Some(value) = Self::proc_array_value(
                                    parent,
                                    v,
                                    run_key.as_str(),
                                    depth + 1,
                                    max_depth,
                                ) {
                                    dat.push(value);
                                }
                            }
                            out.push(DataField::from_arr(run_key, dat));
                            return Ok(());
                        }
                    }
                }

                let mut item_name = String::with_capacity(arr_name.len() + 2 + 10);
                for (i, v) in arr.iter().enumerate() {
                    // 复用 item_name，减少分配
                    item_name.clear();
                    item_name.push_str(arr_name.as_str());
                    item_name.push('[');
                    // 下标转字符串
                    item_name.push_str(i.to_string().as_str());
                    item_name.push(']');
                    Self::proc_value_inner(
                        fpu,
                        upper_sep,
                        parent,
                        v,
                        &item_name[..],
                        false,
                        out,
                        depth + 1,
                        max_depth,
                    )?;
                }
                //out.append(&mut dat);
                return Ok(());
            }
            Value::Object(o) => {
                Self::proc_json_map(
                    fpu,
                    upper_sep,
                    j_path.as_str(),
                    o,
                    name,
                    exact,
                    out,
                    depth + 1,
                    max_depth,
                )?;
                //out.append(&mut sub);
                return Ok(());
            }
        }

        Ok(())
    }

    fn proc_array_value(
        parent: &str,
        v: &Value,
        name: &str,
        depth: usize,
        max_depth: usize,
    ) -> Option<DataField> {
        // 无 fpu，这里仅做上限兜底，保持与对象分支一致性
        if depth > max_depth {
            // 返回结构化错误，但不继续深入；调用方按 None 处理
            return None;
        }
        let j_path = Self::json_path(parent, name);
        match v {
            Value::Null => {
                //no need process
            }
            Value::Bool(b_v) => {
                return Some(DataField::from_bool(name, *b_v));
            }
            Value::Number(num) => {
                if let (true, Some(f)) = (num.is_f64(), num.as_f64()) {
                    return Some(DataField::from_float(name, f));
                } else if num.is_i64() {
                    if let Some(i_n) = num.as_i64() {
                        return Some(DataField::from_digit(name, i_n));
                    }
                } else if num.is_u64() {
                    if let Some(u) = num.as_u64() {
                        if u <= i64::MAX as u64 {
                            return Some(DataField::from_digit(name, u as i64));
                        } else {
                            return Some(DataField::from_chars(name.to_string(), num.to_string()));
                        }
                    }
                } else {
                    return Some(DataField::from_chars(name.to_string(), num.to_string()));
                }
            }
            Value::String(_) => {
                let v_str = v.to_string();
                return Some(DataField::from_chars(
                    name.to_string(),
                    v_str.trim_matches('"').trim().to_string(),
                ));
            }
            Value::Array(arr) => {
                let mut dat = Vec::with_capacity(10);
                for v in arr.iter() {
                    if let Some(field) = Self::proc_array_value(parent, v, "", depth + 1, max_depth)
                    {
                        dat.push(field);
                    }
                }
                return Some(DataField::from_arr(name, dat));
            }
            Value::Object(o) => {
                let mut sub_fields = ObjectValue::default();
                for (k, v) in o {
                    if let Some(value) =
                        Self::proc_array_value(j_path.as_str(), v, k, depth + 1, max_depth)
                    {
                        sub_fields.insert(k.clone(), value);
                    }
                }
                return Some(DataField::new_opt(DataType::Obj, None, sub_fields.into()));
            }
        }
        None
    }
}
