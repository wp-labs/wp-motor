use rusqlite::ToSql;

use super::{SqlNamedParam, ToSqlParams};

impl<'a> ToSqlParams<'a, [(&'a str, &'a dyn ToSql); 1]> for [SqlNamedParam; 1] {
    fn to_params(&'a self) -> [(&'a str, &'a dyn ToSql); 1] {
        [(self[0].0.get_name(), &self[0])]
    }
}

macro_rules! impl_to_params {
    ($n:literal) => {
        impl<'a> ToSqlParams<'a, [(&'a str, &'a dyn ToSql); $n]> for [SqlNamedParam; $n] {
            fn to_params(&'a self) -> [(&'a str, &'a dyn ToSql); $n] {
                let mut params = [("", &"" as &dyn ToSql); $n];
                for (i, param) in self.iter().enumerate() {
                    params[i] = (param.0.get_name(), param);
                }
                params
            }
        }
    };
}

impl_to_params!(2);
impl_to_params!(3);
impl_to_params!(4);
impl_to_params!(5);
impl_to_params!(6);
impl_to_params!(7);
impl_to_params!(8);
impl_to_params!(9);
impl_to_params!(10);
