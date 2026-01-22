pub trait VarAccess {
    fn field_name(&self) -> &Option<String>;
}
