#[macro_export]
macro_rules! for_all_impl {
    ($name: ident,$info_name:ident,$err:ident, $sub_err: expr) => {
        fn $name(self) -> Result<T, $err> {
            match self {
                Ok(x) => Ok(x),
                Err(e) => Err($sub_err(e.to_string())),
            }
        }
        fn $info_name<S: Into<String>>(self, info: S) -> Result<T, $err> {
            match self {
                Ok(x) => Ok(x),
                Err(e) => Err($sub_err(format!("{}:\n{}", info.into(), e))),
            }
        }
    };
}

#[macro_export]
macro_rules! for_simple_impl {
    ($name: ident,$info_name:ident,$err:ident, $sub_err: expr) => {
        fn $name(self) -> Result<T, $err> {
            match self {
                Ok(x) => Ok(x),
                Err(e) => Err($sub_err(e.to_string())),
            }
        }
    };
}

#[macro_export]
macro_rules! for_sys_simple {
    ($err:ident, $sub_err: expr) => {
        $crate::for_simple_impl!(for_sys, for_sys_info, $err, $sub_err);
    };
}
#[macro_export]
macro_rules! for_sys_all {
    ($err:ident, $sub_err: expr) => {
        $crate::for_all_impl!(for_sys, for_sys_info, $err, $sub_err);
    };
}

#[macro_export]
macro_rules! for_logic_simple {
    ($err:ident, $sub_err: expr) => {
        $crate::for_simple_impl!(for_logic, for_sys_logic, $err, $sub_err);
    };
}

#[macro_export]
macro_rules! for_conf_simple {
    ($err:ident, $sub_err: expr) => {
        $crate::for_simple_impl!(for_conf, for_conf_info, $err, $sub_err);
    };
}

#[macro_export]
macro_rules! for_data_simple {
    ($err:ident, $sub_err: expr) => {
        $crate::for_simple_impl!(for_data, for_data_info, $err, $sub_err);
    };
}

#[macro_export]
macro_rules! for_rule_simple {
    ($err:ident, $sub_err: expr) => {
        $crate::for_simple_impl!(for_rule, for_rule_info, $err, $sub_err);
    };
}
#[macro_export]
macro_rules! for_source_simple {
    ($err:ident, $sub_err: expr) => {
        $crate::for_simple_impl!(for_source, for_source_info, $err, $sub_err);
    };
}

#[macro_export]
macro_rules! for_sink_all {
    ($err:ident, $sub_err: expr) => {
        $crate::for_all_impl!(for_sink, for_sink_info, $err, $sub_err);
    };
}
