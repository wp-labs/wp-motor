#[macro_export]
macro_rules! derive_base_prs {
    ($name:ident) => {
        #[derive(Debug, Default)]
        #[allow(dead_code)]
        pub struct $name {}
        /*
        impl $crate::engine::field::parse_def::FieldConfAble for $name
        where
            Self: $crate::engine::field::BaseAble,
        {
            fn meta(&self) -> wp_model_core::model::meta::Meta {
                self.base().field_conf.meta_type.clone()
            }
            fn conf(&self) -> &$crate::core::WPLField {
                &self.base().field_conf
            }
            fn use_conf(&mut self, conf: $crate::core::WPLField) {
                self.mut_base().field_conf = conf;
            }
        }

         */
    };

    ($name:ident, $patten_first: expr) => {
        #[derive(Debug)]
        #[allow(dead_code)]
        pub struct $name {
            base: $crate::engine::field::BasePRS,
        }

        impl $name {
            fn name_tag(&self) -> &str {
                stringify!($name)
            }
        }

        impl $crate::engine::field::BaseAble for $name {
            fn name(&self) -> &str {
                self.name_tag()
            }

            fn base(&self) -> &$crate::engine::field::BasePRS {
                &self.base
            }
            fn mut_base(&mut self) -> &mut $crate::engine::field::BasePRS {
                &mut self.base
            }
        }
        impl orion_overload::::New1<$crate::core::WPLField> for $name {
            fn new(mut conf: $crate::core::WPLField) -> Self {
                if conf.fmt_conf.patten_first.is_none() {
                    conf.fmt_conf.patten_first = Some($patten_first);
                };
                Self {
                    base: $crate::engine::field::BasePRS::new(conf),
                }
            }
        }
        /*
        impl $crate::engine::field::parse_def::FieldConfAble for $name
        where
            Self: $crate::engine::field::BaseAble,
        {
            fn meta(&self) -> wp_model_core::model::meta::Meta {
                self.base().field_conf.meta_type.clone()
            }
            fn conf(&self) -> &$crate::core::WPLField {
                &self.base().field_conf
            }
            fn use_conf(&mut self, conf: $crate::core::WPLField) {
                self.mut_base().field_conf = conf;
            }
        }

         */
    };
}

#[macro_export]
macro_rules! over_loop_check {
    ($var:expr,$max:expr) => {
        if $var > $max {
            break;
        }
        $var += 1;
    };
}
#[macro_export]
macro_rules! true_break {
    ($express:expr) => {
        if $express {
            break;
        }
    };
}
#[macro_export]
macro_rules! option_loop_break {
    ($cur_cnt :expr, $max_val: expr) => {
        if let Some(var) = $max_val {
            if $cur_cnt >= var {
                break;
            }
        }
    };
}

#[macro_export]
macro_rules! true_continue {
    ($express:expr) => {
        if $express {
            continue;
        }
    };
}
