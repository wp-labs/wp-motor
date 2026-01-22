use orion_overload::conv::OptionConv;
use orion_overload::conv::OptionError;
use thiserror::Error;

use crate::ast::WplField;

#[derive(Error, Debug, PartialEq)]
pub enum FieldError {
    #[error("filed define less : {0} ")]
    LessData(String),
    #[error("filed conf less : {0} ")]
    LessConf(String),
    #[error("field is empty ")]
    Empty,
}

impl OptionError for FieldError {
    fn empty() -> Self {
        FieldError::Empty
    }

    fn less(msg: String) -> Self {
        FieldError::LessConf(msg)
    }
}

impl<'a> OptionConv<&'a WplField, FieldError> for Option<&'a WplField> {
    fn no_less(self, name: &str) -> Result<&'a WplField, FieldError> {
        self.ok_or(FieldError::LessConf(format!("{} less", name)))
    }
    fn no_empty(self) -> Result<&'a WplField, FieldError> {
        self.ok_or(FieldError::Empty)
    }
}

#[cfg(test)]
mod test {
    use crate::ast::WplField;
    use crate::eval::value::error::FieldError;
    use orion_overload::conv::OptionConv;

    //test option conv

    #[test]
    fn test_option_conv() {
        let conf = None::<&WplField>.no_empty();
        assert_eq!(conf, Err(FieldError::Empty));
    }
}
